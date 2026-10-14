//! The market's one page (ADR 14.10.26p D2–D4). The widget's markup is the
//! server kit's `widget_snippet`; this file adds the copy around it and a few
//! inline lines for the widget's events. English only: the examples are not
//! localised.

use lingara_embed::server::{SnippetOptions, widget_snippet};

const TITLE: &str = "The Night Market";
const KEEPER: &str = "Mei, at the tea stall, has a practice for you. Finish it and she lights you a lantern.";
pub const ORIGIN_REFUSED: &str = "This site is not on the client's list of websites that can show Lingara.";

/// The widget's two events: a finished practice starts the lantern poll,
/// and an error is shown as a line, `origin_refused` in words of its own.
const EVENTS: &str = r#"<script type="module">
const widget = document.getElementById("lingara-embed");
const status = document.getElementById("status");
widget.addEventListener("lingara:error", (e) => {
  status.textContent = e.detail.code === "origin_refused"
    ? "ORIGIN_REFUSED"
    : `Lingara could not continue: ${e.detail.code}`;
});
widget.addEventListener("lingara:practice-completed", () => {
  status.textContent = "Mei is fetching your lantern…";
  const poll = setInterval(async () => {
    const { lanterns } = await (await fetch("/rewards")).json();
    if (lanterns > 0) {
      status.textContent = `Lanterns: ${lanterns}`;
      clearInterval(poll);
    }
  }, 5000);
});
</script>"#;

/// The page with `<lingara-embed>` on the player's plan, its token from
/// `/token`. A snippet the kit refuses renders as the no-plan page.
pub fn with_widget(plan_id: &str, app_origin: Option<&str>) -> String {
    let options = SnippetOptions {
        title: Some(TITLE.into()),
        theme: Some("auto".into()),
        lingara_origin: app_origin.map(String::from),
        ..SnippetOptions::new(plan_id, "/token")
    };
    match widget_snippet(&options) {
        Ok(snippet) => document(&format!("<p>{KEEPER}</p>\n{snippet}\n<p id=\"status\" role=\"status\"></p>\n{}", EVENTS.replace("ORIGIN_REFUSED", ORIGIN_REFUSED))),
        Err(e) => without_plan(e.field),
    }
}

/// The page with no plan: the code, and no widget.
pub fn without_plan(code: &str) -> String {
    document(&format!("<p>The stalls are closed tonight: Lingara gave no plan (<code>{}</code>).</p>", escape(code)))
}

fn document(body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{TITLE}</title>\n</head>\n<body>\n<h1>{TITLE}</h1>\n{body}\n</body>\n</html>\n"
    )
}

/// The code comes from Lingara's answer, so it is escaped like any text.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
