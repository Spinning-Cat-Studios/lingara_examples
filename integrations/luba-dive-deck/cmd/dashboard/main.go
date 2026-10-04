// Command dashboard is Luba's ops dashboard refresh: it asks for usage:read
// and nothing else, reads the client's usage once, and prints what is left.
//
//	go run ./cmd/dashboard
package main

import (
	"context"
	"fmt"
	"io"
	"os"

	"github.com/Spinning-Cat-Studios/lingara_examples/integrations/luba-dive-deck/internal/env"
)

func main() {
	e, err := env.FromProcess()
	if err == nil {
		err = run(context.Background(), e, os.Stdout)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "dashboard:", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, e env.Env, out io.Writer) error {
	client, err := e.Client("usage:read")
	if err != nil {
		return err
	}
	usage, err := client.GetUsage(ctx)
	if err != nil {
		return err
	}
	for _, row := range usage.Value.Allowance {
		fmt.Fprintf(out, "%-14s %-9s %d of %d left\n", row.Feature, row.Window, row.Remaining, row.Limit)
	}
	if usage.Value.Ledger != nil {
		fmt.Fprintf(out, "metered: %d calls, %d units this month\n", usage.Value.Ledger.Calls, usage.Value.Ledger.Units)
	}
	return nil
}
