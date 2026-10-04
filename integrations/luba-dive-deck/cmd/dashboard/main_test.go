package main

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/Spinning-Cat-Studios/lingara_examples/integrations/luba-dive-deck/internal/env"
)

// TestDashboardAsksOnlyForUsage runs the dashboard against a recorded token
// exchange and usage body: its token request carries scope=usage:read and no
// other scope, and it prints the allowance left.
func TestDashboardAsksOnlyForUsage(t *testing.T) {
	var scopes []string
	stub := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		switch r.Method + " " + r.URL.Path {
		case "POST /oauth/token":
			_ = r.ParseForm()
			scopes = append(scopes, r.PostForm.Get("scope"))
			_, _ = w.Write([]byte(`{"access_token":"lgr_at_dashboard","token_type":"Bearer","expires_in":3600,"scope":"usage:read"}`))
		case "GET /v1/usage":
			_, _ = w.Write([]byte(`{"allowance":[{"feature":"vocab","window":"daily","limit":50,"used":12,"remaining":38,"reset_at":1790208000}]}`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer stub.Close()

	var out strings.Builder
	e := env.Env{ClientID: "lgr_cid_example", ClientSecret: "lgr_cs_example", BaseURL: stub.URL, TokenURL: stub.URL + "/oauth/token"}
	if err := run(context.Background(), e, &out); err != nil {
		t.Fatal(err)
	}
	if len(scopes) != 1 || scopes[0] != "usage:read" {
		t.Fatalf("token requests asked for %q, want exactly [\"usage:read\"]", scopes)
	}
	if !strings.Contains(out.String(), "38 of 50 left") {
		t.Fatalf("printed %q, want the allowance left", out.String())
	}
}
