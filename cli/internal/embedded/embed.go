// Package embedded holds files that are compiled into the lumen-cli binary.
package embedded

import _ "embed"

//go:embed SKILL.md
var SkillMD string
