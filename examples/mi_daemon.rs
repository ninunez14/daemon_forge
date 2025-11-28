use daemon_forge::{DaemonError, ForgeDaemon};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::Duration;

fn main() {
    let pwd = env::current_dir().unwrap();
    let stdout_path = pwd.join("daemon.out");
    let stderr_path = pwd.join("daemon.err");
    let pid_path = pwd.join("daemon.pid");

    println!("--- Launcher DaemonForge ---");
    println!("PID File: {:?}", pid_path);

    // CORRECCIÓN CRÍTICA: Usar append(true) para evitar truncado al reiniciar el proceso.
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stdout_path)
        .expect("No pude abrir stdout log");

    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stderr_path)
        .expect("No pude abrir stderr log");

    let daemon = ForgeDaemon::new()
        .name("mi_servicio_pro")
        .pid_file(&pid_path)
        .working_directory(&pwd)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .inherit_env()
        .env("TEST_MODE", "EXTREME")
        .privileged_action(move || {
            println!("[Setup] Verificando pre-condiciones...");
            if pid_path.exists() {
                println!("[Info] El fichero PID ya existe...");
            }
            Ok("Inicialización Completada")
        });

    match daemon.start() {
        Ok(setup_msg) => {
            // Reabrimos el log en modo append para el proceso hijo
            let mut log_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&stdout_path)
                .unwrap();

            writeln!(
                log_file,
                "[DAEMON: mi_servicio_pro] Iniciado. Setup: {}",
                setup_msg
            )
            .unwrap();

            for i in 1..=3 {
                writeln!(log_file, "⏳ [Fase 1] Tick {}", i).unwrap();
                thread::sleep(Duration::from_secs(1));
            }

            writeln!(log_file, "🔍 [Fase 2] Verificando Variables...").unwrap();
            let mode = env::var("TEST_MODE").unwrap_or_default();

            if mode == "EXTREME" {
                writeln!(log_file, "✅ [Edge Case] Variables OK.").unwrap();
            } else {
                writeln!(log_file, "❌ [Edge Case] Fallo en variables.").unwrap();
            }

            writeln!(
                log_file,
                "💣 [Fase 3] Pánico en 5 segundos (prueba bloqueo ahora)..."
            )
            .unwrap();
            thread::sleep(Duration::from_secs(5));

            eprintln!("🔥 [CRITICAL] Mensaje pre-pánico en stderr.");
            panic!("¡ERROR FATAL SIMULADO!");
        }

        Err(e) => {
            match e {
                DaemonError::TargetLocked => {
                    eprintln!("\n⛔ ERROR: El daemon YA está corriendo (Mutex/PID bloqueado).")
                }
                _ => eprintln!("\n💥 ERROR: {}", e),
            }
            std::process::exit(1);
        }
    }
}
