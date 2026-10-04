// Command dispatch is Luba's dispatch job for one dive: it asks for
// vocab:generate and nothing else, generates the dive's vocabulary, and
// prints each item as the cabin screen would show it.
//
//	go run ./cmd/dispatch [--dive okinawa|jeju|cozumel] [--source-lang en] [--level 2] [--count 5]
package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"os"
	"slices"
	"strings"

	lingara "github.com/Spinning-Cat-Studios/lingara_api_clients/go"

	"github.com/Spinning-Cat-Studios/lingara_examples/integrations/luba-dive-deck/internal/env"
)

// dives maps each destination to the language its divers will hear.
var dives = map[string]string{"okinawa": "ja", "jeju": "ko", "cozumel": "es"}

func main() {
	e, err := env.FromProcess()
	if err == nil {
		err = run(context.Background(), e, os.Stdout, os.Args[1:])
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "dispatch:", err)
		os.Exit(1)
	}
}

// parseDive reads the flags into the request for one dive.
func parseDive(args []string) (string, lingara.VocabRequest, error) {
	flags := flag.NewFlagSet("dispatch", flag.ContinueOnError)
	dive := flags.String("dive", "okinawa", "the destination: okinawa, jeju or cozumel")
	source := flags.String("source-lang", "en", "the divers' own language")
	level := flags.Uint("level", 2, "the level, 1 to 6")
	count := flags.Uint("count", 5, "how many words")
	if err := flags.Parse(args); err != nil {
		return "", lingara.VocabRequest{}, err
	}
	target, ok := dives[*dive]
	if !ok {
		names := make([]string, 0, len(dives))
		for name := range dives {
			names = append(names, name)
		}
		slices.Sort(names)
		return "", lingara.VocabRequest{}, fmt.Errorf("--dive is one of %s", strings.Join(names, ", "))
	}
	n := uint8(*count)
	return *dive, lingara.VocabRequest{SourceLang: *source, TargetLang: target, Level: uint8(*level), Count: &n}, nil
}

func run(ctx context.Context, e env.Env, out io.Writer, args []string) error {
	dive, req, err := parseDive(args)
	if err != nil {
		return err
	}
	client, err := e.Client("vocab:generate")
	if err != nil {
		return err
	}
	stream, err := client.GenerateVocabulary(ctx, req)
	if err != nil {
		return err
	}
	fmt.Fprintf(out, "Today's dive: %s (%s → %s, level %d)\n", dive, req.SourceLang, req.TargetLang, req.Level)
	for event, err := range stream.Events() {
		if err != nil {
			return err
		}
		if item, ok := event.(lingara.GenerateVocabularyEventItem); ok {
			fmt.Fprintln(out, screenLine(item.Data))
		}
	}
	return nil
}

// screenLine is one word as the cabin screen shows it.
func screenLine(item lingara.VocabItem) string {
	line := "  " + item.Word
	if item.Reading != nil {
		line += " (" + *item.Reading + ")"
	} else if item.Pronunciation != nil {
		line += " (" + *item.Pronunciation + ")"
	}
	line += " — " + item.Translation
	if item.Example != nil {
		line += "\n      " + item.Example.Sentence + " / " + item.Example.Translation
	}
	return line
}
