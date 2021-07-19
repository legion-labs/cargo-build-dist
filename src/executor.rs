//! So far nothing has been written to the file system
//! the executor takes the result of the planner and runs it
//! if --no-run is specified the executor early out.

/// trying the use of template, easier than manipulating strings
pub fn render() {
    let tpl_name = "Dockerfile";
    if let Ok(template) = tera::Template::new(
        tpl_name,
        None,
        include_str!("templates/Dockerfile.template"),
    ) {
        let mut tera = tera::Tera::default();
        tera.set_escape_fn(escape_docker);
        tera.autoescape_on(vec!["Dockerfile"]);
        tera.templates.insert(tpl_name.to_string(), template);

        let mut context = tera::Context::new();
        context.insert("base", "ubuntu:20.04");
        context.insert("executable", "cargo-dockerize");

        if let Ok(result) = tera.render(tpl_name, &context) {
            println!("{}", result);
        }
    }
}

fn escape_docker(input: &str) -> String {
    let mut output = String::with_capacity(input.len() * 2);
    for c in input.chars() {
        match c {
            '\n' => output.push_str("\\"),
            '\r' => output.push_str(""),
            _ => output.push(c),
        }
    }

    // Not using shrink_to_fit() on purpose
    output
}