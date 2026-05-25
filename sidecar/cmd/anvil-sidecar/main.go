package main

import (
	"flag"
	"fmt"
	"os"
)

const (
	version    = "0.1.0"
	binaryName = "anvil-sidecar"
)

func main() {
	showVersion := flag.Bool("version", false, "print version and exit")
	flag.Parse()

	if *showVersion {
		fmt.Printf("%s %s\n", binaryName, version)
		os.Exit(0)
	}

	fmt.Fprintf(os.Stderr, "%s %s: no command given (try --version)\n", binaryName, version)
	os.Exit(1)
}
