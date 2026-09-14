package cmd

import (
	"errors"
	"path/filepath"
	"strings"
	"testing"

	"github.com/lumen-dot-app/lumen/cli/internal/exitcodes"
	"github.com/lumen-dot-app/lumen/cli/internal/testutil"
)

func TestDeployInstallDryRun(t *testing.T) {
	t.Setenv("LUMEN_DEPLOYMENT_DIR", "")
	t.Setenv("INSTALL_PREFIX", "")
	ios, out, _ := testutil.TestIOStreams()
	cmd := newDeployInstallCmd(ios)
	cmd.SilenceErrors = true
	cmd.SilenceUsage = true
	// --tag skips the release lookup, so a dry run stays fully offline.
	cmd.SetArgs([]string{"--dry-run", "--tag", "v1.2.3", "--dir", filepath.Join(t.TempDir(), "lumen")})

	if err := cmd.Execute(); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	got := out.String()
	for _, want := range []string{"Dry run complete", "Default image tag: v1.2.3"} {
		if !strings.Contains(got, want) {
			t.Errorf("output missing %q:\n%s", want, got)
		}
	}
}

func TestDeployInstallLegacyFlagsRedirect(t *testing.T) {
	cases := map[string]string{
		"--shutdown":    "lumen-cli deploy stop",
		"--delete-data": "lumen-cli deploy uninstall",
	}
	for flag, want := range cases {
		ios, _, _ := testutil.TestIOStreams()
		cmd := newDeployInstallCmd(ios)
		cmd.SilenceErrors = true
		cmd.SilenceUsage = true
		cmd.SetArgs([]string{flag})

		err := cmd.Execute()
		if err == nil || !strings.Contains(err.Error(), want) {
			t.Errorf("%s: err = %v, want mention of %q", flag, err, want)
			continue
		}
		var exitErr *exitcodes.ExitError
		if !errors.As(err, &exitErr) || exitErr.Code != exitcodes.BadRequest {
			t.Errorf("%s: exit code = %v, want BadRequest", flag, err)
		}
	}
}

func TestDeployInstallRejectsLiteWithCraft(t *testing.T) {
	ios, _, _ := testutil.TestIOStreams()
	cmd := newDeployInstallCmd(ios)
	cmd.SilenceErrors = true
	cmd.SilenceUsage = true
	cmd.SetArgs([]string{"--lite", "--include-craft", "--dry-run"})

	err := cmd.Execute()
	if err == nil || !strings.Contains(err.Error(), "cannot be used together") {
		t.Fatalf("err = %v", err)
	}
}

func TestInstallLumenIsAnAlias(t *testing.T) {
	ios, _, _ := testutil.TestIOStreams()
	alias := newInstallLumenCmd(ios)
	if alias.Use != "install-lumen" {
		t.Fatalf("Use = %q", alias.Use)
	}
	// Same flag surface as deploy install.
	for _, name := range []string{"lite", "include-craft", "tag", "local", "offline", "no-prompt", "dry-run", "verbose", "no-wait", "dir", "force"} {
		if alias.Flags().Lookup(name) == nil {
			t.Errorf("alias missing --%s", name)
		}
	}
}
