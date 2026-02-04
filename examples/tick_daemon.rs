use daemon_forge::ForgeDaemon;
use std::fs::OpenOptions;
use std::thread;
use std::time::Duration;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use signal_hook::consts::signal::*;
use signal_hook::flag;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Definir rutas
    let pwd = env::current_dir().unwrap();
    let log_path = pwd.join("ticker.log");
    let err_path = pwd.join("ticker.err");
    let pid_path = pwd.join("ticker.pid");

    // 2. Abrir archivos de log (Append mode)
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
    println!("Logs will be written to: {:?}", log_path);

    // 3. Configurar el Daemon
    let daemon = ForgeDaemon::new()
        .name("tick_daemon")
        .pid_file(&pid_path)
        .working_directory(&pwd)
        .stdout(stdout_file)
        .stderr(stderr_file)
        // AQUÍ está el cambio principal: Pasamos la lógica como un closure
        .privileged_action(move || {
            
            // --- INICIO CÓDIGO DEL DAEMON ---
            
            // A. Configurar manejo de señales para parada limpia (SIGTERM / SIGINT)
            let term = Arc::new(AtomicBool::new(false));
            flag::register(SIGTERM, Arc::clone(&term))?; // Systemd Stop
            flag::register(SIGINT, Arc::clone(&term))?;  // Ctrl+C (Manual)

            println!("[Ticker] Servicio iniciado. PID: {}", std::process::id());

            let frases = [
                "Estan vivos",
                "I have no mouth",
                "I must scream",
                "Hello world",
                "Adios mundo :0"
            ];

            let mut i = 0;

            // B. Bucle principal que respeta la señal de parada
            while !term.load(Ordering::Relaxed) {
                let frase = frases[i % frases.len()];
                
                // C. Usamos println!: La librería ya redirigió esto al archivo .log
                println!("[Ticker] Ping #{} - {}", i, frase);
                
                i += 1;
                
                // Dormir en intervalos cortos para revisar la señal de parada frecuentemente
                thread::sleep(Duration::from_secs(3));
            }

            println!("[Ticker] Señal de parada recibida. Cerrando limpiamente.");
            Ok(())
        });

    // 4. Arrancar usando la lógica refactorizada (Systemd o Fork)
    // Nota: Usamos la función del módulo unix directamente
    daemon_forge::ForgeDaemon::start(daemon)?;

    Ok(())
}