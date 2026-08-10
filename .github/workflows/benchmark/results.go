package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

const (
	// Budget for the extension loaded but with no tracer attached. This is the
	// near-zero-cost claim, so it gets the tight one - the point of a gate is to
	// fail when the claim stops being true.
	EnabledP95Budget = 0.05

	// Budget with a tracer attached. Instrumentation costs real time here, and
	// always-installed observer handlers mean it costs more than it used to.
	ProbingP95Budget = 0.25

	// Deltas smaller than this are runner noise whatever the percentage, so both
	// the relative and the absolute test must trip before we fail.
	AbsFloorMs = 10.0
)

// The subset of k6's --summary-export we depend on.
type Report struct {
	RootGroup struct {
		Checks struct {
			OK struct {
				Passes float64 `json:"passes"`
				Fails  float64 `json:"fails"`
			} `json:"ok"`
		} `json:"checks"`
	} `json:"root_group"`
	Metrics struct {
		HTTPReqDuration struct {
			Avg float64 `json:"avg"`
			P95 float64 `json:"p(95)"`
		} `json:"http_req_duration"`
	} `json:"metrics"`
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run() error {
	// Directory holding one subdirectory per configuration, each with a
	// report.json. Defaults to the working directory so the tool can be run by
	// hand from wherever the reports were collected.
	base := "."
	if len(os.Args) > 1 {
		base = os.Args[1]
	}

	control, err := getReport(base, "control")
	if err != nil {
		return err
	}

	enabled, err := getReport(base, "enabled")
	if err != nil {
		return err
	}

	probing, err := getReport(base, "probing")
	if err != nil {
		return err
	}

	var errs []error

	// A configuration that returned errors has not measured anything meaningful,
	// so compare nothing until every request succeeded. The old gate tolerated
	// five failures, which on ~600 requests is a broken extension, not slack.
	for name, report := range map[string]Report{"control": control, "enabled": enabled, "probing": probing} {
		if fails := report.RootGroup.Checks.OK.Fails; fails > 0 {
			errs = append(errs, fmt.Errorf("%s: %d request(s) failed their check", name, int(fails)))
		}
	}

	enabledDiff := enabled.Metrics.HTTPReqDuration.P95 - control.Metrics.HTTPReqDuration.P95
	probingDiff := probing.Metrics.HTTPReqDuration.P95 - control.Metrics.HTTPReqDuration.P95

	errs = append(errs, checkBudget("enabled", enabledDiff, control.Metrics.HTTPReqDuration.P95, EnabledP95Budget))
	errs = append(errs, checkBudget("probing", probingDiff, control.Metrics.HTTPReqDuration.P95, ProbingP95Budget))

	summary := fmt.Sprintf(`| Test    | Avg     | p95     | p95 diff | p95 diff %% | Budget |
|---------|---------|---------|----------|------------|--------|
| Control | %.1fms | %.1fms | | | |
| Enabled | %.1fms | %.1fms | %+.1fms | %+.1f%% | %.0f%% |
| Probing | %.1fms | %.1fms | %+.1fms | %+.1f%% | %.0f%% |

Deltas under %.0fms are treated as runner noise and do not fail the build.
`,
		control.Metrics.HTTPReqDuration.Avg, control.Metrics.HTTPReqDuration.P95,
		enabled.Metrics.HTTPReqDuration.Avg, enabled.Metrics.HTTPReqDuration.P95,
		enabledDiff, percent(enabledDiff, control.Metrics.HTTPReqDuration.P95), EnabledP95Budget*100,
		probing.Metrics.HTTPReqDuration.Avg, probing.Metrics.HTTPReqDuration.P95,
		probingDiff, percent(probingDiff, control.Metrics.HTTPReqDuration.P95), ProbingP95Budget*100,
		AbsFloorMs,
	)

	if err := os.WriteFile("summary.md", []byte(summary), 0644); err != nil {
		errs = append(errs, err)
	}

	return errors.Join(errs...)
}

func checkBudget(name string, diff, baseline, budget float64) error {
	if diff <= AbsFloorMs {
		return nil
	}
	if ratio := diff / baseline; ratio > budget {
		return fmt.Errorf(
			"%s: p95 is %+.1fms (%+.1f%%) over control, budget is %.0f%%",
			name, diff, ratio*100, budget*100,
		)
	}
	return nil
}

func percent(diff, baseline float64) float64 {
	if baseline == 0 {
		return 0
	}
	return diff / baseline * 100
}

func getReport(base, configuration string) (Report, error) {
	var report Report

	path := filepath.Join(base, configuration, "report.json")

	file, err := os.Open(path)
	if err != nil {
		return report, fmt.Errorf(
			"failed to open %s (the %s run probably did not complete - check the k6 step): %w",
			path, configuration, err,
		)
	}

	defer file.Close()

	data, err := io.ReadAll(file)
	if err != nil {
		return report, fmt.Errorf("failed to read %s: %w", path, err)
	}

	if err := json.Unmarshal(data, &report); err != nil {
		return report, fmt.Errorf("failed to unmarshal %s: %w", path, err)
	}

	// encoding/json leaves absent fields at zero, so a k6 schema change would
	// otherwise yield avg=0, diff=0, fails=0 for every configuration - a gate
	// that passes unconditionally, including when every request 500s. Refuse to
	// report on a file we evidently did not understand.
	if report.Metrics.HTTPReqDuration.Avg == 0 || report.Metrics.HTTPReqDuration.P95 == 0 {
		return report, fmt.Errorf(
			"%s: http_req_duration avg/p95 parsed as zero - the k6 summary schema has probably changed",
			path,
		)
	}

	if report.RootGroup.Checks.OK.Passes == 0 {
		return report, fmt.Errorf(
			"%s: no passing checks found - the k6 summary schema has probably changed, or every request failed",
			path,
		)
	}

	return report, nil
}
