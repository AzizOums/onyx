package provider

import (
	"context"
	"fmt"
	"testing"

	"github.com/hashicorp/terraform-plugin-testing/helper/resource"
	"github.com/hashicorp/terraform-plugin-testing/terraform"
	"github.com/lumen-dot-app/lumen/terraform-provider-lumen/internal/client"
)

func TestAccConnectorResource(t *testing.T) {
	resource.Test(t, resource.TestCase{
		PreCheck:                 func() { testAccPreCheck(t) },
		ProtoV6ProviderFactories: testAccProtoV6ProviderFactories,
		CheckDestroy:             testAccCheckConnectorDestroyed(t),
		Steps: []resource.TestStep{
			{
				// A web connector needs no credential to be created, so this
				// exercises the resource without touching a live source.
				Config: `
resource "lumen_connector" "test" {
  name       = "tf-acc-connector"
  source     = "web"
  input_type = "load_state"
  connector_specific_config = jsonencode({
    base_url           = "https://example.com"
    web_connector_type = "single"
  })
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_connector.test", "name", "tf-acc-connector"),
					resource.TestCheckResourceAttr("lumen_connector.test", "source", "web"),
					resource.TestCheckResourceAttr("lumen_connector.test", "credential_ids.#", "0"),
					resource.TestCheckResourceAttrSet("lumen_connector.test", "id"),
				),
			},
			{
				ResourceName:      "lumen_connector.test",
				ImportState:       true,
				ImportStateVerify: true,
			},
			{
				// Renaming and rescheduling must not replace the connector, and
				// the server's prune_freq default must not show up as drift.
				Config: `
resource "lumen_connector" "test" {
  name         = "tf-acc-connector-renamed"
  source       = "web"
  input_type   = "load_state"
  refresh_freq = 86400
  connector_specific_config = jsonencode({
    base_url           = "https://example.com"
    web_connector_type = "single"
  })
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_connector.test", "name", "tf-acc-connector-renamed"),
					resource.TestCheckResourceAttr("lumen_connector.test", "refresh_freq", "86400"),
					resource.TestCheckResourceAttr("lumen_connector.test", "prune_freq", "604800"),
				),
			},
		},
	})
}

func testAccCheckConnectorDestroyed(t *testing.T) resource.TestCheckFunc {
	return func(s *terraform.State) error {
		c := testAccClient(t)
		for name, rs := range s.RootModule().Resources {
			if rs.Type != "lumen_connector" {
				continue
			}
			id, err := parseIDString(rs.Primary.ID)
			if err != nil {
				return err
			}
			if _, err := c.GetConnector(context.Background(), id); !client.IsNotFound(err) {
				return fmt.Errorf("%s: connector %s still exists after destroy (err: %v)", name, rs.Primary.ID, err)
			}
		}
		return nil
	}
}
