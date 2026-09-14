package api

import "fmt"

// LumenAPIError is returned when an Lumen API call fails.
type LumenAPIError struct {
	StatusCode int
	Detail     string
}

func (e *LumenAPIError) Error() string {
	return fmt.Sprintf("HTTP %d: %s", e.StatusCode, e.Detail)
}

// AuthError is returned when authentication or authorization fails.
type AuthError struct {
	Message string
}

func (e *AuthError) Error() string {
	return e.Message
}
