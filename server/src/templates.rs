use askama::Template;

// Template for the index page
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

// Template for the manual page
#[derive(Template)]
#[template(path = "manual.html")]
pub struct ManualTemplate;