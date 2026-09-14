#!/bin/sh
# Import by numeric group id.
#
# The seeded Admin and Basic groups can be imported and their rosters managed,
# but Lumen refuses to rename or delete one, or to change its permissions.
terraform import lumen_user_group.data_platform 4
