package install

import (
	"context"

	"github.com/lumen-dot-app/lumen/cli/internal/deploy/dockercmd"
	"github.com/lumen-dot-app/lumen/cli/internal/deploy/paths"
	"github.com/lumen-dot-app/lumen/cli/internal/exitcodes"
)

// RunStop implements `deploy stop` (install.sh --shutdown): pause the
// containers without removing them.
func RunStop(ctx context.Context, deps Deps, opts Options) error {
	in := newInstaller(deps, opts)
	return in.runStop(ctx)
}

func (in *installer) runStop(ctx context.Context) error {
	in.root = paths.Resolve(in.opts.Dir)
	if !in.hasComposeFile() {
		in.warnf("No Lumen deployment found at %s. Nothing to shut down.", in.root.Dir)
		return nil
	}
	in.resolveProjectFromDisk()

	if err := in.attachDockerCompose(ctx); err != nil {
		return err
	}

	in.infof("Stopping Lumen containers...")
	// Overlays are auto-detected from disk so users don't need to remember
	// which flags they installed with; HOST_PORT/IMAGE_TAG fall back to safe
	// defaults because compose only needs them to resolve the file, not to
	// stop containers (same pair install.sh uses here).
	cmd := in.compose.Command(in.deploymentDir(), stopFallbackEnv(), in.composeFileNames(true), "stop")
	cmd.Stdout, cmd.Stderr = in.deps.IOS.Out, in.deps.IOS.ErrOut
	if _, err := in.deps.Runner.Run(ctx, cmd); err != nil {
		in.errorf("Failed to stop containers")
		return exitcodes.Newf(exitcodes.General, "docker compose stop failed: %v", err)
	}
	in.successf("Lumen containers stopped (paused)")
	in.plainf("")
	in.infof("Start them again with: %s", in.paint.Accent("lumen-cli deploy install"))
	return nil
}

// attachDockerCompose wires the docker/compose handles for lifecycle verbs
// that must not provision anything (stop/status/uninstall).
func (in *installer) attachDockerCompose(ctx context.Context) error {
	if !dockercmd.Installed() {
		return exitcodes.New(exitcodes.General, "docker not found. Is Docker installed?")
	}
	in.docker.RefreshSudo(ctx)
	if in.docker.UsingSudo() {
		in.infof("Using sudo for docker commands in this run.")
	}
	in.compose = dockercmd.DetectCompose(ctx, in.docker)
	if in.compose == nil {
		return exitcodes.New(exitcodes.General, "Docker Compose not found.\n  Visit: https://docs.docker.com/compose/install/")
	}
	in.compose.Project = in.projectName()
	return nil
}
