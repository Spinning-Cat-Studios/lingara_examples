package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/Spinning-Cat-Studios/lingara_examples/integrations/luba-dive-deck/internal/env"
)

// One recorded vocabulary stream: started, two items, done.
const vocabStream = "event: started\ndata: {\"meta\":{\"level\":2,\"source_lang\":\"en\",\"target_lang\":\"ja\",\"framework\":\"JLPT\",\"count\":2,\"ai_generated\":true}}\n\n" +
	"event: item\ndata: {\"word\":\"海\",\"reading\":\"うみ\",\"translation\":\"sea\",\"example\":{\"sentence\":\"海はきれいです。\",\"translation\":\"The sea is beautiful.\"}}\n\n" +
	"event: item\ndata: {\"word\":\"魚\",\"reading\":\"さかな\",\"translation\":\"fish\"}\n\n" +
	"event: done\ndata: {}\n\n"

// TestDispatchAsksOnlyForVocab runs one Okinawa dive against a recorded token
// exchange and vocabulary stream: its token request carries
// scope=vocab:generate and no other scope, it asks for Japanese, and it
// prints each word.
func TestDispatchAsksOnlyForVocab(t *testing.T) {
	var scopes []string
	var asked map[string]any
	stub := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.Method + " " + r.URL.Path {
		case "POST /oauth/token":
			_ = r.ParseForm()
			scopes = append(scopes, r.PostForm.Get("scope"))
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"access_token":"lgr_at_dispatch","token_type":"Bearer","expires_in":3600,"scope":"vocab:generate"}`))
		case "POST /v1/vocab/stream":
			_ = json.NewDecoder(r.Body).Decode(&asked)
			w.Header().Set("Content-Type", "text/event-stream")
			_, _ = w.Write([]byte(vocabStream))
		default:
			http.NotFound(w, r)
		}
	}))
	defer stub.Close()

	var out strings.Builder
	e := env.Env{ClientID: "lgr_cid_example", ClientSecret: "lgr_cs_example", BaseURL: stub.URL, TokenURL: stub.URL + "/oauth/token"}
	if err := run(context.Background(), e, &out, []string{"--dive", "okinawa"}); err != nil {
		t.Fatal(err)
	}
	if len(scopes) != 1 || scopes[0] != "vocab:generate" {
		t.Fatalf("token requests asked for %q, want exactly [\"vocab:generate\"]", scopes)
	}
	if asked["target_lang"] != "ja" {
		t.Fatalf("asked for %v, want target_lang ja for okinawa", asked)
	}
	for _, want := range []string{"海 (うみ) — sea", "魚 (さかな) — fish"} {
		if !strings.Contains(out.String(), want) {
			t.Fatalf("printed %q, want %q", out.String(), want)
		}
	}
}

func TestDispatchRefusesAnUnknownDive(t *testing.T) {
	if _, _, err := parseDive([]string{"--dive", "atlantis"}); err == nil {
		t.Fatal("an unknown dive was accepted")
	}
}
