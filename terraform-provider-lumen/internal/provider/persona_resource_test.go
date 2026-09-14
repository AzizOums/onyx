package provider

import (
	"context"
	"fmt"
	"regexp"
	"testing"

	"github.com/hashicorp/terraform-plugin-testing/helper/acctest"
	"github.com/hashicorp/terraform-plugin-testing/helper/resource"
	"github.com/hashicorp/terraform-plugin-testing/terraform"
)

// An action for the agent to hold, so tool_ids is exercised against a real id
// rather than a literal.
const personaDependencies = `
resource "lumen_custom_tool" "persona" {
  name       = "tf-acc-persona-action"
  definition = jsonencode({
    openapi = "3.0.0"
    info = {
      title       = "Weather"
      description = "Looks up the weather"
    }
    servers = [{ url = "https://api.example.com" }]
    paths = {
      "/weather" = {
        get = {
          operationId = "getWeather"
          summary     = "Get the current weather"
          responses   = { "200" = { description = "ok" } }
        }
      }
    }
  })
}
`

func TestAccPersonaResource(t *testing.T) {
	// Agent names are unique across the deployment and a delete only leaves a
	// tombstone, so a run that dies before its cleanup would block every later
	// run on the name. A per-run name keeps the suite repeatable.
	name := acctest.RandomWithPrefix("tf-acc-agent")

	resource.Test(t, resource.TestCase{
		PreCheck:                 func() { testAccPreCheck(t) },
		ProtoV6ProviderFactories: testAccProtoV6ProviderFactories,
		CheckDestroy:             testAccCheckPersonaDestroyed(t),
		Steps: []resource.TestStep{
			{
				Config: personaDependencies + `
resource "lumen_persona" "test" {
  name          = "` + name + `"
  description   = "Answers support questions"
  system_prompt = "You are a support agent. Be brief."
  task_prompt   = "Answer using the handbook."
  tool_ids      = [lumen_custom_tool.persona.id]
  is_listed     = false

  # Lumen ignores this on an update, so changing it in the next step proves
  # the follow-up call to the display-priority endpoint runs.
  display_priority = 5

  starter_messages = [
    {
      name    = "Refunds"
      message = "How do refunds work?"
    },
    {
      name    = "Shipping"
      message = "How long does shipping take?"
    },
  ]
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_persona.test", "name", name),
					resource.TestCheckResourceAttr("lumen_persona.test", "description", "Answers support questions"),
					resource.TestCheckResourceAttr("lumen_persona.test", "system_prompt", "You are a support agent. Be brief."),
					resource.TestCheckResourceAttr("lumen_persona.test", "task_prompt", "Answer using the handbook."),
					resource.TestCheckResourceAttr("lumen_persona.test", "is_public", "true"),
					// A new agent is always listed, so this proves the
					// follow-up call to the listed endpoint ran.
					resource.TestCheckResourceAttr("lumen_persona.test", "is_listed", "false"),
					resource.TestCheckResourceAttr("lumen_persona.test", "is_featured", "false"),
					resource.TestCheckResourceAttr("lumen_persona.test", "display_priority", "5"),
					resource.TestCheckResourceAttr("lumen_persona.test", "builtin_persona", "false"),
					resource.TestCheckResourceAttr("lumen_persona.test", "tool_ids.#", "1"),
					resource.TestCheckResourceAttr("lumen_persona.test", "starter_messages.#", "2"),
					resource.TestCheckResourceAttr("lumen_persona.test", "starter_messages.0.name", "Refunds"),
					resource.TestCheckResourceAttrSet("lumen_persona.test", "id"),
					// Optional collections left unset must stay null, or every
					// plan would report a change back to null.
					resource.TestCheckNoResourceAttr("lumen_persona.test", "document_set_ids.#"),
					resource.TestCheckNoResourceAttr("lumen_persona.test", "users.#"),
					resource.TestCheckNoResourceAttr("lumen_persona.test", "groups.#"),
					resource.TestCheckNoResourceAttr("lumen_persona.test", "icon_name"),
				),
			},
			{
				// A full-replace update: rename, drop the description and the
				// task prompt, detach the action, drop the starter messages,
				// and list it again.
				Config: personaDependencies + `
resource "lumen_persona" "test" {
  name          = "` + name + `-renamed"
  system_prompt = "You are a support agent. Be thorough."
  tool_ids      = []
  is_listed     = true
  is_featured   = true
  icon_name     = "user"

  display_priority = 2
}
`,
				Check: resource.ComposeAggregateTestCheckFunc(
					resource.TestCheckResourceAttr("lumen_persona.test", "name", name+"-renamed"),
					resource.TestCheckResourceAttr("lumen_persona.test", "description", ""),
					resource.TestCheckResourceAttr("lumen_persona.test", "system_prompt", "You are a support agent. Be thorough."),
					resource.TestCheckResourceAttr("lumen_persona.test", "task_prompt", ""),
					resource.TestCheckResourceAttr("lumen_persona.test", "is_listed", "true"),
					resource.TestCheckResourceAttr("lumen_persona.test", "display_priority", "2"),
					// Featuring an agent needs agent-management permission.
					resource.TestCheckResourceAttr("lumen_persona.test", "is_featured", "true"),
					resource.TestCheckResourceAttr("lumen_persona.test", "icon_name", "user"),
					resource.TestCheckResourceAttr("lumen_persona.test", "tool_ids.#", "0"),
					// Dropped from the configuration rather than emptied, so
					// it goes back to null. Checking a count of "0" would not
					// prove that: the helper treats "0" and absent alike.
					resource.TestCheckNoResourceAttr("lumen_persona.test", "starter_messages.#"),
				),
			},
			{
				ResourceName:      "lumen_persona.test",
				ImportState:       true,
				ImportStateVerify: true,
			},
		},
	})
}

// Agent names are unique, and a create that lands on a live name is refused
// rather than quietly taking the agent over.
func TestAccPersonaResourceRejectsADuplicateName(t *testing.T) {
	name := acctest.RandomWithPrefix("tf-acc-duplicate")

	resource.Test(t, resource.TestCase{
		PreCheck:                 func() { testAccPreCheck(t) },
		ProtoV6ProviderFactories: testAccProtoV6ProviderFactories,
		CheckDestroy:             testAccCheckPersonaDestroyed(t),
		Steps: []resource.TestStep{
			{
				Config: `
resource "lumen_persona" "first" {
  name = "` + name + `"
}
`,
			},
			{
				Config: `
resource "lumen_persona" "first" {
  name = "` + name + `"
}

resource "lumen_persona" "second" {
  name = "` + name + `"
}
`,
				ExpectError: regexp.MustCompile(`(?s)already exists`),
			},
		},
	})
}

func testAccCheckPersonaDestroyed(t *testing.T) resource.TestCheckFunc {
	return func(state *terraform.State) error {
		c := testAccClient(t)
		for name, rs := range state.RootModule().Resources {
			if rs.Type != "lumen_persona" {
				continue
			}
			id, err := parseIDString(rs.Primary.ID)
			if err != nil {
				return err
			}
			_, found, err := c.LookupPersona(context.Background(), id)
			if err != nil {
				return fmt.Errorf("unexpected error reading %s after destroy: %w", name, err)
			}
			if found {
				return fmt.Errorf("%s (id %d) still exists after destroy", name, id)
			}
		}
		return nil
	}
}
