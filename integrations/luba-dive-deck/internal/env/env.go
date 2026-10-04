// Package env is the environment both commands read: the same four
// variables every example in this repository uses, so a developer sets them
// once.
package env

import (
	"errors"
	"os"

	lingara "github.com/Spinning-Cat-Studios/lingara_api_clients/go"
)

// Env is the client's credentials and, optionally, where Lingara is.
type Env struct {
	ClientID     string
	ClientSecret string
	// BaseURL and TokenURL override production when set: a local backend,
	// or the offline tests' stub.
	BaseURL  string
	TokenURL string
}

// FromProcess reads LINGARA_CLIENT_ID, LINGARA_CLIENT_SECRET,
// LINGARA_BASE_URL and LINGARA_TOKEN_URL.
func FromProcess() (Env, error) {
	e := Env{
		ClientID:     os.Getenv("LINGARA_CLIENT_ID"),
		ClientSecret: os.Getenv("LINGARA_CLIENT_SECRET"),
		BaseURL:      os.Getenv("LINGARA_BASE_URL"),
		TokenURL:     os.Getenv("LINGARA_TOKEN_URL"),
	}
	if e.ClientID == "" || e.ClientSecret == "" {
		return Env{}, errors.New("set LINGARA_CLIENT_ID and LINGARA_CLIENT_SECRET")
	}
	return e, nil
}

// Client asks for exactly scopes, and nothing else the client may hold.
func (e Env) Client(scopes ...string) (*lingara.Client, error) {
	opts := []lingara.Option{
		lingara.WithClientCredentials(e.ClientID, e.ClientSecret),
		lingara.WithScopes(scopes...),
	}
	if e.BaseURL != "" {
		opts = append(opts, lingara.WithBaseURL(e.BaseURL))
	}
	if e.TokenURL != "" {
		opts = append(opts, lingara.WithTokenURL(e.TokenURL))
	}
	return lingara.New(opts...)
}
