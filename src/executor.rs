//! So far nothing has been written to the file system
//! the executor takes the result of the planner and runs it
//! if --no-run is specified the executor early out.

/// trying the use of template, easier than manipulating strings
pub fn render(actions: Vec<Box<dyn crate::Action>>) {
    for action in actions {
        if let Err(e) = action.run(){
            println!("failed in render {}", e);
        }
    }
}

pub fn dryrun_render(actions: Vec<Box<dyn crate::Action>>){
    for action in actions{
        if let Err(e) = action.dryrun(){
            println!("Error in dry_render {}", e);
        }
    }
}

