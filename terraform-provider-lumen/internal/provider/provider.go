// Package provider implements the Terraform provider for Lumen application
// configuration.
package provider

import (
	"context"
	"fmt"
	"os"

	"github.com/hashicorp/terraform-plugin-framework/datasource"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/provider"
	"github.com/hashicorp/terraform-plugin-framework/provider/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/lumen-dot-app/lumen/terraform-provider-lumen/internal/client"
)

var _ provider.Provider = (*lumenProvider)(nil)

type lumenProvider struct {
	version string
}

// New returns a provider factory, as expected by providerserver.Serve and the
// acceptance-test harness.
func New(version string) func() provider.Provider {
	return func() provider.Provider {
		return &lumenProvider{version: version}
	}
}

type lumenProviderModel struct {
	Endpoint  types.String `tfsdk:"endpoint"`
	APIKey    types.String `tfsdk:"api_key"`
	APIPrefix types.String `tfsdk:"api_prefix"`
}

func (p *lumenProvider) Metadata(_ context.Context, _ provider.MetadataRequest, resp *provider.MetadataResponse) {
	resp.TypeName = "lumen"
	resp.Version = p.version
}

func (p *lumenProvider) Schema(_ context.Context, _ provider.SchemaRequest, resp *provider.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "Manage Lumen application configuration (LLM providers, API keys, " +
			"workspace settings, ...) declaratively via the Lumen admin API.",
		Attributes: map[string]schema.Attribute{
			"endpoint": schema.StringAttribute{
				Optional: true,
				MarkdownDescription: "Lumen server origin, e.g. `https://cloud.lumen.app` or `http://localhost:3000`. " +
					"May also be set via the `LUMEN_SERVER_URL` environment variable.",
			},
			"api_key": schema.StringAttribute{
				Optional:  true,
				Sensitive: true,
				MarkdownDescription: "Lumen API key (`on_...`) in the seeded `Admin` group, or unrestricted " +
					"personal access token (`lumen_pat_...`). May also be set via the `LUMEN_API_KEY` " +
					"environment variable.",
			},
			"api_prefix": schema.StringAttribute{
				Optional: true,
				MarkdownDescription: "Path prefix the API is served under, defaults to `/api` (the web proxy). " +
					"Set to `\"\"` when talking directly to the backend (e.g. `http://localhost:8080`). " +
					"May also be set via the `LUMEN_API_PREFIX` environment variable.",
			},
		},
	}
}

func (p *lumenProvider) Configure(ctx context.Context, req provider.ConfigureRequest, resp *provider.ConfigureResponse) {
	var config lumenProviderModel
	resp.Diagnostics.Append(req.Config.Get(ctx, &config)...)
	if resp.Diagnostics.HasError() {
		return
	}

	// Values derived from not-yet-applied resources are unknown during plan;
	// reject them instead of silently treating them as empty/known.
	for name, value := range map[string]types.String{
		"endpoint":   config.Endpoint,
		"api_key":    config.APIKey,
		"api_prefix": config.APIPrefix,
	} {
		if value.IsUnknown() {
			resp.Diagnostics.AddAttributeError(
				path.Root(name),
				"Unknown provider configuration value",
				fmt.Sprintf("The provider cannot be configured with an unknown %s. "+
					"Apply the resource it derives from first, or set it via its environment variable.", name),
			)
		}
	}
	if resp.Diagnostics.HasError() {
		return
	}

	endpoint := os.Getenv("LUMEN_SERVER_URL")
	if !config.Endpoint.IsNull() {
		endpoint = config.Endpoint.ValueString()
	}
	apiKey := os.Getenv("LUMEN_API_KEY")
	if !config.APIKey.IsNull() {
		apiKey = config.APIKey.ValueString()
	}
	apiPrefix := "/api"
	if v, ok := os.LookupEnv("LUMEN_API_PREFIX"); ok {
		apiPrefix = v
	}
	if !config.APIPrefix.IsNull() {
		apiPrefix = config.APIPrefix.ValueString()
	}

	if endpoint == "" {
		resp.Diagnostics.AddError(
			"Missing Lumen endpoint",
			"Set the provider's endpoint attribute or the LUMEN_SERVER_URL environment variable.",
		)
	}
	if apiKey == "" {
		resp.Diagnostics.AddError(
			"Missing Lumen API key",
			"Set the provider's api_key attribute or the LUMEN_API_KEY environment variable. "+
				"Create an API key in the Lumen admin panel, assigned to the Admin group "+
				"(or via POST /admin/api-key with group_ids set to the Admin group's id).",
		)
	}
	if resp.Diagnostics.HasError() {
		return
	}

	c := client.NewClient(client.Config{
		ServerURL: endpoint,
		APIPrefix: apiPrefix,
		APIKey:    apiKey,
		Version:   p.version,
	})
	resp.ResourceData = c
	resp.DataSourceData = c
}

func (p *lumenProvider) Resources(_ context.Context) []func() resource.Resource {
	return []func() resource.Resource{
		NewAPIKeyResource,
		NewLLMProviderResource,
		NewLLMProviderDefaultResource,
		NewSettingsResource,
		NewEmbeddingProviderResource,
		NewCredentialResource,
		NewConnectorResource,
		NewCCPairResource,
		NewDocumentSetResource,
		NewCustomToolResource,
		NewPersonaResource,
		NewMCPServerResource,
		NewUserGroupResource,
	}
}

func (p *lumenProvider) DataSources(_ context.Context) []func() datasource.DataSource {
	return []func() datasource.DataSource{
		NewLLMProvidersDataSource,
		NewEmbeddingProvidersDataSource,
		NewSettingsDataSource,
		NewConnectorsDataSource,
	}
}

// clientFromResourceConfigure extracts the shared *client.Client in a
// resource's Configure, adding a diagnostic on type mismatch.
func clientFromResourceConfigure(req resource.ConfigureRequest, resp *resource.ConfigureResponse) *client.Client {
	if req.ProviderData == nil {
		return nil // Configure is called before provider Configure during validation.
	}
	c, ok := req.ProviderData.(*client.Client)
	if !ok {
		resp.Diagnostics.AddError(
			"Unexpected resource configure type",
			fmt.Sprintf("Expected *client.Client, got %T. This is a bug in the provider.", req.ProviderData),
		)
		return nil
	}
	return c
}

// clientFromDataSourceConfigure is clientFromResourceConfigure for data sources.
func clientFromDataSourceConfigure(req datasource.ConfigureRequest, resp *datasource.ConfigureResponse) *client.Client {
	if req.ProviderData == nil {
		return nil
	}
	c, ok := req.ProviderData.(*client.Client)
	if !ok {
		resp.Diagnostics.AddError(
			"Unexpected data source configure type",
			fmt.Sprintf("Expected *client.Client, got %T. This is a bug in the provider.", req.ProviderData),
		)
		return nil
	}
	return c
}
