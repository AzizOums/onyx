package provider

import (
	"context"
	"fmt"
	"testing"

	"github.com/hashicorp/terraform-plugin-testing/helper/resource"
	"github.com/hashicorp/terraform-plugin-testing/terraform"
	"github.com/lumen-dot-app/lumen/terraform-provider-lumen/internal/client"
)

// ccPairDependencies is the connector and credential every pair needs.
//
// The source is mock_connector because creating a pair runs the connector's
// real validate_connector_settings, which reaches the source system. Lumen
// short-circuits that check for mock_connector and ingestion_api, so the test
// exercises the whole lifecycle without a live source.
const ccPairDependencies = `
resource "lumen_connector" "test" {
  name                      = "tf-acc-ccpair-connector"
  source                    = "mock_connector"
  input_type                = "poll"
  connector_specific_config = jsonencode({})
}

resource "lumen_credential" "test" {
  name            = "tf-acc-ccpair-credential"
  source          = "mock_connector"
  credential_json = jsonencode({})
}
`

func TestAccCCPairResource(t *testing.T) {
	resource.Test(t, resource.TestCase{
		PreCheck:                 func() { testAccPreCheck(t) },
		ProtoV6ProviderFactories: testAccProtoV6ProviderFactories,
		CheckDestroy:             testAccCheckCCPairDestroyed(t),
		Steps: []resource.TestStep{
			{
				Config: ccPairDependencies + `
resource "lumen_cc_pair" "test" {
  name          = "tf-acc-ccpair"
  connector_id  = lumen_connector.test.id
  credential_id = lumen_credential.test.id
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "name", "tf-acc-ccpair"),
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "access_type", "public"),
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "paused", "false"),
					resource.TestCheckResourceAttrSet("lumen_cc_pair.test", "id"),
					resource.TestCheckResourceAttrSet("lumen_cc_pair.test", "status"),
					// The pair must carry its own id, not the connector's.
					resource.TestCheckResourceAttrPair(
						"lumen_cc_pair.test", "connector_id", "lumen_connector.test", "id"),
					resource.TestCheckResourceAttrPair(
						"lumen_cc_pair.test", "credential_id", "lumen_credential.test", "id"),
					testAccCheckCCPairIsNotConnectorID(),
				),
			},
			{
				ResourceName:      "lumen_cc_pair.test",
				ImportState:       true,
				ImportStateVerify: true,
				// Lumen cycles these on its own as indexing progresses, so they
				// change between the apply and the import.
				ImportStateVerifyIgnore: []string{
					"status", "last_index_attempt_status", "num_docs_indexed",
				},
			},
			{
				// Rename and pause in one apply. Neither may replace the pair,
				// which would delete and re-index everything it holds.
				Config: ccPairDependencies + `
resource "lumen_cc_pair" "test" {
  name          = "tf-acc-ccpair-renamed"
  connector_id  = lumen_connector.test.id
  credential_id = lumen_credential.test.id
  paused        = true
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "name", "tf-acc-ccpair-renamed"),
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "paused", "true"),
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "status", "PAUSED"),
				),
			},
			{
				// Resuming must report the server's new status, not "ACTIVE"
				// guessed from the request.
				Config: ccPairDependencies + `
resource "lumen_cc_pair" "test" {
  name          = "tf-acc-ccpair-renamed"
  connector_id  = lumen_connector.test.id
  credential_id = lumen_credential.test.id
  paused        = false
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_cc_pair.test", "paused", "false"),
					resource.TestCheckResourceAttrWith("lumen_cc_pair.test", "status", func(value string) error {
						if value == "PAUSED" || value == "DELETING" {
							return fmt.Errorf("a resumed pair should not report status %q", value)
						}
						return nil
					}),
				),
			},
		},
	})
}

// testAccCheckCCPairIsNotConnectorID guards the create endpoint's no-op
// branch, which answers success=false and returns the connector id.
func testAccCheckCCPairIsNotConnectorID() resource.TestCheckFunc {
	return func(s *terraform.State) error {
		pair, ok := s.RootModule().Resources["lumen_cc_pair.test"]
		if !ok {
			return fmt.Errorf("lumen_cc_pair.test is not in state")
		}
		connector, ok := s.RootModule().Resources["lumen_connector.test"]
		if !ok {
			return fmt.Errorf("lumen_connector.test is not in state")
		}
		if pair.Primary.ID == connector.Primary.ID {
			return fmt.Errorf("the pair id equals the connector id (%s): the no-op create response was stored", pair.Primary.ID)
		}
		return nil
	}
}

// testAccCheckCCPairDestroyed proves the destroy waited for Celery. The row
// survives a deletion attempt until the background task clears it, so a
// destroy that returned early would still find the pair here.
func testAccCheckCCPairDestroyed(t *testing.T) resource.TestCheckFunc {
	return func(s *terraform.State) error {
		c := testAccClient(t)
		for name, rs := range s.RootModule().Resources {
			if rs.Type != "lumen_cc_pair" {
				continue
			}
			id, err := parseIDString(rs.Primary.ID)
			if err != nil {
				return err
			}
			pair, err := c.GetCCPair(context.Background(), id)
			if client.IsNotFound(err) {
				continue
			}
			if err != nil {
				return fmt.Errorf("checking whether %s was destroyed: %w", name, err)
			}
			return fmt.Errorf("%s still exists with status %s", name, pair.Status)
		}
		return nil
	}
}
