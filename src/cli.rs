//! Concrete command implementations for init, setup, config, status.

use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;
use inquire::{Confirm, Select, Text};

use crate::config::{
    ensure_anvil_dir, load_config, save_config, AnvilConfig, CredentialRef, ModelBinding, ProviderConnection, Roles,
};

/// `anvil init`
pub fn cmd_init(root: &Path) -> Result<()> {
    let cfg_path = root.join("anvil.toml");
    if cfg_path.exists() {
        println!("{} already initialized at {}", "anvil".green(), root.display());
        return cmd_config_show(root);
    }

    ensure_anvil_dir(root)?;

    let mut cfg = AnvilConfig::default();

    // Seed a couple of example providers so the user sees the shape immediately.
    cfg.providers.insert(
        "local-ollama".to_string(),
        ProviderConnection {
            r#type: "openai_compat".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            // No key required for stock Ollama. The client supplies a conventional placeholder.
            credential: CredentialRef::None,
            extra: Default::default(),
        },
    );

    cfg.providers.insert(
        "anthropic".to_string(),
        ProviderConnection {
            r#type: "anthropic".to_string(),
            base_url: None,
            credential: CredentialRef::Keyring,
            extra: Default::default(),
        },
    );

    cfg.providers.insert(
        "openai".to_string(),
        ProviderConnection {
            r#type: "openai_compat".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            credential: CredentialRef::Keyring,
            extra: Default::default(),
        },
    );

    save_config(root, &cfg)?;

    println!(
        "{} Initialized Anvil project at {}",
        "✓".green(),
        root.display()
    );
    println!("  Created anvil.toml + .anvil/");
    println!("  Run {} to configure your providers and model bindings.", "`anvil setup`".cyan());
    println!("  Then: {}  →  {}  → work in phases with forced R1+R2 reviews.", "`anvil talk`".cyan(), "`anvil plan`".cyan());
    Ok(())
}

/// `anvil setup` — the most important onboarding experience.
pub fn cmd_setup(root: &Path) -> Result<()> {
    let mut cfg = load_config(root).unwrap_or_default();
    ensure_anvil_dir(root)?;

    println!("\n{}", "=== Anvil Setup ===".bold());
    println!("We'll configure providers (how you reach models) and then bind specific models to roles.");
    println!("The key rule: reviewer-a and reviewer-b should be *different models from different providers*.\n");

    // 1. Provider connections loop
    loop {
        let add = Confirm::new("Add / update a provider connection?")
            .with_default(true)
            .prompt()?;

        if !add {
            break;
        }

        let name: String = Text::new("Connection name (e.g. local-ollama, my-anthropic, azure-east):")
            .prompt()?;

        let ptype = Select::new(
            "Provider type",
            vec![
                "openai_compat  (Ollama, Groq, Together, Fireworks, OpenRouter, Azure compat, vLLM, ...)",
                "anthropic",
                "google",
                "azure_openai (native, or use openai_compat)",
                "aws_bedrock    (not yet native — use gateway or openai_compat if available)",
                "other (you will enter the string)",
            ],
        )
        .prompt()?;

        let ptype = if ptype.starts_with("openai_compat") {
            "openai_compat".to_string()
        } else if ptype.starts_with("azure_openai") {
            "azure_openai".to_string()
        } else if ptype.starts_with("aws_bedrock") {
            "aws_bedrock".to_string()
        } else if ptype.starts_with("other") {
            Text::new("Enter provider type string:").prompt()?
        } else {
            ptype.split_whitespace().next().unwrap().to_string()
        };

        let base_url = if ptype == "openai_compat" || ptype == "azure_openai" {
            Some(
                Text::new("Base URL (press enter for default OpenAI):")
                    .with_default("https://api.openai.com/v1")
                    .prompt()?,
            )
        } else if ptype == "anthropic" {
            None // default is fine
        } else {
            Text::new("Base URL (optional):").prompt().ok().filter(|s| !s.trim().is_empty())
        };

        let cred_choice = Select::new(
            "How will the API key be provided?",
            vec![
                "Store in OS keyring (recommended for real providers)",
                "Environment variable",
                "No authentication required (local Ollama, unauthenticated self-hosted, etc.)",
            ],
        )
        .prompt()?;

        let credential = if cred_choice.contains("No authentication") {
            CredentialRef::None
        } else if cred_choice.contains("keyring") {
            CredentialRef::Keyring
        } else {
            let var = Text::new("Environment variable name:").prompt()?;
            CredentialRef::Env { var_name: var }
        };

        let conn = ProviderConnection {
            r#type: ptype,
            base_url,
            credential,
            extra: Default::default(),
        };

        cfg.providers.insert(name.clone(), conn);
        println!("  {} Added provider connection '{}'", "✓".green(), name);

        // Immediately ask for the key if using keyring
        if matches!(cfg.providers[&name].credential, CredentialRef::Keyring) {
            let key = inquire::Password::new(&format!("API key / token for '{}':", name))
                .without_confirmation()
                .prompt()?;
            let entry_name = format!("provider:{}", name);
            let entry = keyring::Entry::new("anvil", &entry_name)?;
            entry.set_password(&key)?;
            println!("  {} Credential stored in keyring", "✓".green());
        }
    }

    if cfg.providers.is_empty() {
        println!("No providers configured. You can re-run `anvil setup` later.");
    }

    // 2. Model bindings
    println!("\n{}", "Now create model bindings (logical names you will reference in roles).".bold());

    loop {
        let add = Confirm::new("Add a model binding?").with_default(!cfg.model_bindings.is_empty()).prompt()?;
        if !add {
            break;
        }

        let name: String = Text::new("Binding name (e.g. coder-claude, reviewer-gemini, local-llama):").prompt()?;

        if cfg.providers.is_empty() {
            return Err(anyhow!("Add at least one provider connection first."));
        }

        let provider_names: Vec<String> = cfg.providers.keys().cloned().collect();
        let provider = Select::new("Use which provider connection?", provider_names).prompt()?;

        let model: String = Text::new("Model identifier (exact string the provider expects):").prompt()?;

        let note: Option<String> = Text::new("Short note (optional, e.g. 'strong at architecture reviews'):")
            .prompt()
            .ok()
            .filter(|s| !s.trim().is_empty());

        cfg.model_bindings.insert(
            name.clone(),
            ModelBinding {
                provider,
                model,
                note,
            },
        );
        println!("  {} Added binding '{}'", "✓".green(), name);
    }

    // 3. Role assignment — this is where the "exactly two different reviewers" contract is made explicit.
    println!("\n{}", "Assign roles. This is the most important part for fighting drift.".bold());
    println!("You should pick two *different* reviewer bindings that use different providers/families when possible.");

    let binding_names: Vec<String> = cfg.model_bindings.keys().cloned().collect();
    if binding_names.is_empty() {
        println!("No bindings yet — skipping role assignment. Run `anvil setup` again later.");
    } else {
        let coder = Select::new("Coder / primary writer (used for implementation work):", binding_names.clone()).prompt()?;
        let planner = Select::new("Planner (plan generation + talk):", binding_names.clone()).prompt()?;

        let reviewer_a = Select::new("Reviewer A (first review round):", binding_names.clone()).prompt()?;
        let reviewer_b = Select::new("Reviewer B (second review round — should be different from A):", binding_names.clone()).prompt()?;

        if reviewer_a == reviewer_b {
            println!("{}", "Warning: reviewer-a and reviewer-b are the same binding. This weakens the anti-drift guarantee.".yellow());
        }

        cfg.roles = Roles {
            coder: Some(coder),
            planner: Some(planner),
            reviewer_a: Some(reviewer_a),
            reviewer_b: Some(reviewer_b),
        };
    }

    save_config(root, &cfg)?;
    println!("\n{} Setup complete. Configuration saved to anvil.toml", "✓".green());
    println!("Next steps:");
    println!("  {}              — have a conversation to capture intent", "`anvil talk`".cyan());
    println!("  {}              — generate plan + forced R1 + R2 reviews", "`anvil plan`".cyan());
    println!("  {} <id>         — start working on a phase", "`anvil phase start`".cyan());
    Ok(())
}

pub fn cmd_config_show(root: &Path) -> Result<()> {
    let cfg = load_config(root)?;

    println!("{}", "Anvil Configuration".bold());
    println!();

    println!("{}", "Roles:".underline());
    println!("  coder:      {}", cfg.roles.coder.as_deref().unwrap_or("(not set)"));
    println!("  planner:    {}", cfg.roles.planner.as_deref().unwrap_or("(not set)"));
    println!("  reviewer-a: {}", cfg.roles.reviewer_a.as_deref().unwrap_or("(not set)"));
    println!("  reviewer-b: {}", cfg.roles.reviewer_b.as_deref().unwrap_or("(not set)"));
    println!();

    println!("{}", "Providers:".underline());
    if cfg.providers.is_empty() {
        println!("  (none — run `anvil setup`)");
    } else {
        for (name, p) in &cfg.providers {
            let base = p.base_url.as_deref().unwrap_or("<default>");
            let auth = match &p.credential {
                CredentialRef::None => "auth=none".to_string(),
                CredentialRef::Keyring => "auth=keyring".to_string(),
                CredentialRef::Env { var_name } => format!("auth=env:{}", var_name),
            };
            println!("  {} — type={}, base={}, {}", name, p.r#type, base, auth);
        }
    }
    println!();

    println!("{}", "Model Bindings:".underline());
    if cfg.model_bindings.is_empty() {
        println!("  (none — run `anvil setup`)");
    } else {
        for (name, b) in &cfg.model_bindings {
            let note = b.note.as_deref().map(|n| format!(" ({})", n)).unwrap_or_default();
            println!("  {} → {} via {}{}", name, b.model, b.provider, note);
        }
    }

    Ok(())
}

pub fn cmd_config_add_provider(root: &Path) -> Result<()> {
    // Just delegate to the interactive setup for now — keeps the UX consistent.
    cmd_setup(root)
}

pub fn cmd_status(root: &Path) -> Result<()> {
    let cfg = match load_config(root) {
        Ok(c) => c,
        Err(_) => {
            println!("Not an Anvil project (no anvil.toml). Run `anvil init`.");
            return Ok(());
        }
    };

    println!("{}", "Project Status".bold());
    println!("Root: {}", root.display());

    let has_roles = cfg.roles.coder.is_some() && cfg.roles.reviewer_a.is_some() && cfg.roles.reviewer_b.is_some();
    if !has_roles {
        println!("{}", "Roles are not fully configured — run `anvil setup`.".yellow());
    } else {
        println!(
            "Reviewers: {} vs {} (different bindings = {})",
            cfg.roles.reviewer_a.as_deref().unwrap_or("?"),
            cfg.roles.reviewer_b.as_deref().unwrap_or("?"),
            if cfg.roles.reviewer_a != cfg.roles.reviewer_b { "yes".green() } else { "NO — bad for drift".red() }
        );
    }

    // Very light state peek
    let state_path = crate::config::state_path(root);
    if state_path.exists() {
        println!("State file present: {}", state_path.display());
    } else {
        println!("No phase state yet (run `anvil plan` then `anvil phase start ...`).");
    }

    Ok(())
}
