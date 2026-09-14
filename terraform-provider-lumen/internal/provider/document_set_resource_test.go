package provider

import (
	"context"
	"fmt"
	"testing"

	"github.com/hashicorp/terraform-plugin-testing/helper/resource"
	"github.com/hashicorp/terraform-plugin-testing/terraform"
	"github.com/lumen-dot-app/lumen/terraform-provider-lumen/internal/client"
)

// documentSetDependencies builds a pair for the set to hold. The names differ
// from the cc-pair test's so a leaked resource cannot collide.
const documentSetDependencies = `
resource "lumen_connector" "docset" {
  name                      = "tf-acc-docset-connector"
  source                    = "mock_connector"
  input_type                = "poll"
  connector_specific_config = jsonencode({})
}

resource "lumen_credential" "docset" {
  name            = "tf-acc-docset-credential"
  source          = "mock_connector"
  credential_json = jsonencode({})
}

resource "lumen_cc_pair" "docset" {
  name          = "tf-acc-docset-ccpair"
  connector_id  = lumen_connector.docset.id
  credential_id = lumen_credential.docset.id
}

# A second pair on the same connector, to prove cc_pair_ids is a full replace.
resource "lumen_credential" "docset_second" {
  name            = "tf-acc-docset-credential-second"
  source          = "mock_connector"
  credential_json = jsonencode({})
}

resource "lumen_cc_pair" "docset_second" {
  name          = "tf-acc-docset-ccpair-second"
  connector_id  = lumen_connector.docset.id
  credential_id = lumen_credential.docset_second.id
}
`

func TestAccDocumentSetResource(t *testing.T) {
	resource.Test(t, resource.TestCase{
		PreCheck:                 func() { testAccPreCheck(t) },
		ProtoV6ProviderFactories: testAccProtoV6ProviderFactories,
		CheckDestroy:             testAccCheckDocumentSetDestroyed(t),
		Steps: []resource.TestStep{
			{
				Config: documentSetDependencies + `
resource "lumen_document_set" "test" {
  name        = "tf-acc-docset"
  description = "Documents for the acceptance test"
  cc_pair_ids = [lumen_cc_pair.docset.id]
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_document_set.test", "name", "tf-acc-docset"),
					resource.TestCheckResourceAttr("lumen_document_set.test", "description", "Documents for the acceptance test"),
					resource.TestCheckResourceAttr("lumen_document_set.test", "is_public", "true"),
					resource.TestCheckResourceAttr("lumen_document_set.test", "cc_pair_ids.#", "1"),
					resource.TestCheckResourceAttrSet("lumen_document_set.test", "id"),
					// Optional collections left unset must stay null, or every
					// plan would report a change back to null.
					resource.TestCheckNoResourceAttr("lumen_document_set.test", "users.#"),
					resource.TestCheckNoResourceAttr("lumen_document_set.test", "groups.#"),
				),
			},
			{
				// This runs directly after the create on purpose. A new set is
				// left syncing, and Lumen rejects a change to a syncing set, so
				// the update has to wait for convergence before it writes.
				//
				// A full-replace update: rename, drop the description, make it
				// private, and swap the pair it holds. Lumen rejects a set with
				// no connectors at all, so the list is replaced, not emptied.
				Config: documentSetDependencies + `
resource "lumen_document_set" "test" {
  name        = "tf-acc-docset-renamed"
  cc_pair_ids = [lumen_cc_pair.docset_second.id]
  is_public   = false
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_document_set.test", "name", "tf-acc-docset-renamed"),
					resource.TestCheckResourceAttr("lumen_document_set.test", "description", ""),
					resource.TestCheckResourceAttr("lumen_document_set.test", "is_public", "false"),
					resource.TestCheckResourceAttr("lumen_document_set.test", "cc_pair_ids.#", "1"),
					resource.TestCheckTypeSetElemAttrPair(
						"lumen_document_set.test", "cc_pair_ids.*", "lumen_cc_pair.docset_second", "id"),
				),
			},
			{
				ResourceName:      "lumen_document_set.test",
				ImportState:       true,
				ImportStateVerify: true,
				// The background sync flips this on its own.
				ImportStateVerifyIgnore: []string{"is_up_to_date"},
			},
		},
	})
}

// testAccCheckDocumentSetDestroyed proves the destroy waited. Delete only
// marks the set, and the name is unique, so returning early would make the
// next apply fail on a name still in use.
func testAccCheckDocumentSetDestroyed(t *testing.T) resource.TestCheckFunc {
	return func(s *terraform.State) error {
		c := testAccClient(t)
		for name, rs := range s.RootModule().Resources {
			if rs.Type != "lumen_document_set" {
				continue
			}
			id, err := parseIDString(rs.Primary.ID)
			if err != nil {
				return err
			}
			set, err := c.GetDocumentSet(context.Background(), id)
			if client.IsNotFound(err) {
				continue
			}
			if err != nil {
				return fmt.Errorf("checking whether %s was destroyed: %w", name, err)
			}
			return fmt.Errorf("%s still exists as %q", name, set.Name)
		}
		return nil
	}
}
