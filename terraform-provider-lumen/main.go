// terraform-provider-lumen manages Lumen application configuration via the
// admin API (for the infrastructure under Lumen, see deployment/terraform).
package main

import (
	"context"
	"flag"
	"log"

	"github.com/hashicorp/terraform-plugin-framework/providerserver"
	"github.com/lumen-dot-app/lumen/terraform-provider-lumen/internal/provider"
)

// Docs generate from schema descriptions + examples/ (needs terraform CLI).
// Pinned: CI fails on any docs/ diff, and an unpinned generator would move that
// output on its own.
//go:generate go run github.com/hashicorp/terraform-plugin-docs/cmd/tfplugindocs@v0.25.0 generate

// version is set via ldflags on release builds.
var version = "dev"

func main() {
	var debug bool
	flag.BoolVar(&debug, "debug", false, "run the provider with support for debuggers like delve")
	flag.Parse()

	err := providerserver.Serve(context.Background(), provider.New(version), providerserver.ServeOpts{
		Address: "registry.terraform.io/lumen-dot-app/lumen",
		Debug:   debug,
	})
	if err != nil {
		log.Fatal(err)
	}
}
