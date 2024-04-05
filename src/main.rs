#![feature(lazy_cell)]
use chumsky::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

mod div;
mod expr;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut rl = DefaultEditor::new()?;
    loop {
        let readline = rl.readline("calq> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                let exp = match expr::expr_parser().parse(line) {
                    Ok(exp) => exp,
                    Err(e) => {
                        for e in e {
                            eprintln!("Error: {e}");
                        }
                        continue;
                    }
                };

                let value = exp.eval();

                match value {
                    Ok(value) => {
                        println!("{value}");
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
    Ok(())
}
