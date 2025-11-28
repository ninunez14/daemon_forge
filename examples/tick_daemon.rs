use daemon_forge::ForgeDaemon;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::Duration;
use std::env;

fn main() {
    // Define where the logs will be stored
    let pwd = env::current_dir().unwrap();
    let log_path = pwd.join("ticker.log");
    let err_path = pwd.join("ticker.err");
    let pid_path = pwd.join("ticker.pid");

    // (Optional) We open them in append mode so we dont erase the history
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Couldn't open stdout");

    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&err_path)
        .expect("Couldn't open stderr");

    println!("Launching a simple ticker Daemon...");
    println!("Look at ticker.log to see the activity.");

    let daemon = ForgeDaemon::new()
        .name("simple_ticker") 
        .pid_file(&pid_path)   
        .working_directory(&pwd)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .start();

    match daemon {
        Ok(_) => {
            // --- DAEMON CODE  ---
            
            // We reopen the file to writo to it in the main loop
            // (Optional: you could use println! as stdout is redirected already)
            let mut log = OpenOptions::new().append(true).open(&log_path).unwrap();
            
            let frases = [
                "Estan vivos",
                "I have no mouth",
                "I must screem",
                "Hello world",
                "Adios mundo :0"
            ];

            let mut i = 0;
            loop {
                let frase = frases[i % frases.len()];
                // Usamos writeln! para asegurar que se escribe en disco
                if let Err(e) = writeln!(log, "[Ticker] Ping #{} - {}", i, frase) {
                    eprintln!("Error writting on the log: {}", e); // Irá a ticker.err
                }
                
                i += 1;
                thread::sleep(Duration::from_secs(3));
            }
        }
        Err(e) => {
            eprintln!("Error : {}", e);
            std::process::exit(1);
        }
    }
}