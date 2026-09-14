package cmd

import (
	"errors"

	"github.com/lumen-dot-app/lumen/cli/internal/api"
	"github.com/lumen-dot-app/lumen/cli/internal/config"
	"github.com/lumen-dot-app/lumen/cli/internal/exitcodes"
)

func requireConfig() (config.LumenCliConfig, error) {
	cfg := config.Load()
	if !cfg.IsConfigured() {
		return cfg, exitcodes.New(exitcodes.NotConfigured,
			"lumen CLI is not configured\n  Set LUMEN_PAT (and optionally LUMEN_SERVER_URL), or run: lumen-cli chat to complete first-time setup")
	}
	return cfg, nil
}

func requireClient() (config.LumenCliConfig, *api.Client, error) {
	cfg, err := requireConfig()
	if err != nil {
		return cfg, nil, err
	}
	return cfg, api.NewClient(cfg), nil
}

func apiErrorToExit(err error, action string) error {
	var authErr *api.AuthError
	if errors.As(err, &authErr) {
		return exitcodes.Newf(exitcodes.AuthFailure, "%s: %v", action, err)
	}
	var apiErr *api.LumenAPIError
	if errors.As(err, &apiErr) {
		return exitcodes.Newf(exitcodes.ForHTTPStatus(apiErr.StatusCode), "%s: %s", action, apiErr.Error())
	}
	return exitcodes.Newf(exitcodes.Unreachable, "%s: %v", action, err)
}
