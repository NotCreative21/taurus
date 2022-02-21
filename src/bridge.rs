use regex::Regex;
use std::io::{BufReader, BufRead};
use std::process::Command;
use std::fs::File;
use lupus::*;
use rcon_rs::{Client, PacketType};
use warp::ws::Message;

pub async fn send_to_discord(clients: &Clients, msg: String) {
    let locked = clients.lock().await;
    for (key, _) in locked.iter() {
        match locked.get(key) {
            Some(t) => {
                if let Some(t) = &t.sender {
                    let _ = t.send(
                        Ok(Message::text(
                                "CHAT_OUT ".to_owned() + 
                                &replace_formatting(
                                    msg.to_owned()))));
                }
            }
            None => continue,
        };
    }
}

// update messages from the log file, this takes in the log file, checks if the lines can be
// ignored, then checks if the new lines are in game commands, if they are then use handle command
// to check them and if not send them to discord
//
// unfortunately this is not very efficient but honestly I don't really care, this runs on separate
// threads from the mc server and if the log file gets above 2k lines it gets repiped with tmux to
// prevent the function from taing too long
pub async fn update_messages(server_name: String, lines: usize) -> (String, usize) {
    let file_path: String = format!("/tmp/{server_name}-lupus");
    if !check_exist(&file_path.to_owned()) { return ("".to_string(), 0); }

    // open the log file in bufreader
    let file = File::open(&file_path).unwrap();
    let reader = BufReader::new(file);
    let mut message = "".to_string();

    let mut cur_line: usize = lines;

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for (i, line) in reader.lines().enumerate() {
        // skip lines that are irrelevant
        if i > cur_line {
            // if they are new, update the counter
            cur_line = i;

            let line = line.unwrap();

            // if the line is too short then skip it
            if &line.chars().count() < &35 { continue; }

            // check if the message starts with certain characters
            let line_sep: &str = &line[33..];
            if !line.starts_with("[") || 
                (!line_sep.starts_with("<") && !line_sep.starts_with("§"))
            {
                continue;
            }

            let newline = &line[33..];

            if newline.len() < 1 { continue; }

            // if it's not an in game command, we can generate what the discord message will be
            //
            // firstly we put the server name then the new line message, this is where replace
            // formatting comes in to remove the special mc escape sequences
            let nmessage = format!("[{server_name}]{newline}\n");

            message.push_str(&nmessage);
        }
    }

    // if the lines are under 2k, we don't need to replace the file since it doesn't take much time
    // to process in the first place
    if lines < 2000 {
        return (message, cur_line);
    }

    // if it is above 2k however, we can reset the pipe and notify the to the console
    gen_pipe(server_name.to_owned(), true).await;
    println!("*info: pipe file reset -> {server_name}");

    // return new line count to update the one in the main file
    ("".to_string(), 0)
}

// checks the number of lines in the log file to set them initially, this prevents old messages
// from being spat out if the bot restarts (and makes it a lot less annoying)
pub fn set_lines(server_name: String) -> usize {
    let file_path: String = format!("/tmp/{}-HypnosCore", &server_name);
    let file = File::open(&file_path).unwrap();
    let reader = BufReader::new(file);

    // count the amount of lines in the log file
    reader.lines().count()
}


pub async fn create_rcon_connections(session: Vec<Session>, msg: String) -> Result<()> {
    for i in session {
        let rcon = match i.rcon {
            Some(v) => v,
            None => continue 
        };

        let ip = match rcon.to_owned().ip {
            Some(t) => t,
            None => "127.0.0.1".to_string(),
        };

        let mut conn = Client::new(&ip, &rcon.to_owned().port.to_string());
        let _ = conn.auth(&rcon.password);

        let _ = conn.send(&msg, Some(PacketType::Cmd));
    }
    Ok(())
}

// This removes all the formmating codes coming from MC chat with regex
#[inline(always)]
pub fn replace_formatting(mut msg: String) -> String {
    msg = msg.replace("_", "\\_");
    // regex to replace any '§' followed by digits with a blank space
    let mc_codes = Regex::new(r"§.*\d").unwrap();
    mc_codes.replace_all(&msg, "").to_owned().to_string()
}

// small function to send a command to the specific tmux session, this replaces new lines due to it
// causing a problem with commands
//
// this is one of the limitations of this system, but it's not that bad because if there are
// multiple lines you can send the command multiple times
#[inline(always)]
pub async fn send_command(server_name: String, message: String) {
    // if there are any non ascii characters then we can return as there's likely problems with the
    // rest of the command
    message.chars().for_each(|c| if !c.is_ascii() { return; });

    Command::new("tmux")
        .args(["send-keys", "-t", &server_name, &message, "Enter"])
        .spawn()
        .expect("*error: failed to send to tmux session");

    reap();
}


// generate the tmux pipe connecting to the specified server, this also takes in the option to
// delete the file if it exists before generating it
// that can be used at startup or when just resetting the file in general
#[inline]
pub async fn gen_pipe(server_name: String, rm: bool) {
    let pipe = format!("/tmp/{}-lupus", &server_name);
    if rm {
        // remove the old pipe file if it exists
        if check_exist(&pipe) {
            Command::new("rm")
                .arg(&pipe)
                .spawn()
                .expect("*error: failed to delete pipe file");
        }
    }

    // create the tmux command that will be entered to set the pipe
    Command::new("tmux")
        .args(["pipe-pane", "-t", &server_name, &format!("cat > {pipe}")])
        .spawn()
        .expect("*error: failed to generate pipe file");

    // call reap to remove any zombie processes generated by it
    reap();
}


