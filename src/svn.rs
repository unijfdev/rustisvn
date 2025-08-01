use ratatui::style::{Color, Style};
use std::collections::HashSet;
use std::hash::Hash;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SvnStatusEntry {
    pub file: PathBuf,
    pub state: String,
}

impl SvnStatusEntry {
    pub fn new(file: PathBuf, state: String) -> Self {
        SvnStatusEntry { file, state }
    }
}

#[derive(Debug, Default)]
pub struct SvnStatusList {
    pub entries: Vec<SvnStatusEntry>,
    pub selections: HashSet<usize>,
    commit_message: String,
}

impl SvnStatusList {
    pub fn new(entries: Vec<SvnStatusEntry>, selections: HashSet<usize>) -> Self {
        SvnStatusList {
            entries,
            selections,
            commit_message: String::new(),
        }
    }

    pub fn commit_message(&self) -> &str {
        &self.commit_message
    }

    pub fn toggle_selection(&mut self, idx: usize) {
        if self.selections.contains(&idx) {
            self.selections.remove(&idx);
        } else {
            self.selections.insert(idx);
        }
    }

    pub fn toggle_selection_by_file(&mut self, idx_selected: usize) {
        let file_to_remove = self
            .selections
            .iter()
            .filter_map(|&idx| self.entries.get(idx))
            .nth(idx_selected)
            .map(|entry| entry.file.to_path_buf());
        if let Some(file) = file_to_remove {
            if let Some(idx) = self.entries.iter().position(|entry| entry.file == file) {
                self.selections.remove(&idx);
            }
        }
    }

    pub fn clear_commit_message(&mut self) {
        self.commit_message.clear();
    }

    pub fn push_char_to_commit_message(&mut self, c: char) {
        self.commit_message.push(c);
    }

    pub fn pop_char_from_commit_message(&mut self) {
        self.commit_message.pop();
    }

    pub fn set_commit_message(&mut self, message: String) {
        self.commit_message = message;
    }
}

#[derive(Debug)]
pub struct SvnClient {
    working_copy: PathBuf,
    pub status: SvnStatusList,
}

impl SvnClient {
    pub fn new<T: AsRef<Path>>(working_copy: T) -> Self {
        SvnClient {
            working_copy: working_copy.as_ref().to_path_buf(),
            status: SvnStatusList::new(Vec::new(), HashSet::new()),
        }
    }

    pub fn raw_command(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new("svn")
            .args(args)
            .current_dir(&self.working_copy)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        match out {
            Ok(o) => {
                if o.status.success() {
                    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
                } else {
                    Err(String::from_utf8_lossy(&o.stdout).into_owned())
                }
            }
            Err(e) => Err(format!("Fallo al ejecutar el comando SVN: {}", e)),
        }
    }

    pub fn svn_status(&self) -> Vec<SvnStatusEntry> {
        let out_result = self.raw_command(&["status"]);
        match out_result {
            Ok(out_string) => {
                let mut entries: Vec<SvnStatusEntry> = out_string
                    .lines()
                    .filter_map(|line| {
                        let mut parts =
                            line.splitn(2, |c: char| c.is_whitespace() && c != '\n' && c != '\r'); // Use a more robust split
                        let state = parts.next()?.to_string();
                        let file_str = parts.next()?.trim(); // Trim whitespace from the file path
                        let file = PathBuf::from(file_str);
                        Some(SvnStatusEntry::new(file, state))
                    })
                    .collect();
                entries.sort_by(|a, b| a.file.cmp(&b.file));
                entries
            }
            Err(e) => {
                eprintln!("Error al obtener el estado de SVN: {}", e);
                Vec::new()
            }
        }
    }

    pub fn init_svn_status(&mut self) {
        let entries = self.svn_status();
        self.status = SvnStatusList::new(entries, HashSet::new());
    }

    pub fn refresh_svn_status(&mut self) {
        let new_entries = self.svn_status();
        let previously_selected_files: HashSet<PathBuf> = self
            .status
            .selections
            .iter()
            .filter_map(|&idx| self.status.entries.get(idx))
            .filter_map(|entry| entry.file.to_str())
            .map(PathBuf::from)
            .collect();
        let mut new_selections = HashSet::new();
        for (new_idx, entry) in new_entries.iter().enumerate() {
            if previously_selected_files.contains(&entry.file) {
                new_selections.insert(new_idx);
            }
        }
        let current_commit_message = self.status.commit_message.clone();
        self.status = SvnStatusList::new(new_entries, new_selections);
        self.status.set_commit_message(current_commit_message);
    }

    pub fn push_basic_commit(&mut self) -> Result<bool, String> {
        let mut args = vec!["commit", "-m", self.status.commit_message()];
        if self.status.commit_message().trim().is_empty() {
            return Err("El mensaje de commit no puede estar vacío.".to_string());
        }
        if self.status.selections.is_empty() {
            return Err("No se han seleccionado archivos para el commit.".to_string());
        }
        let file_args: Vec<&str> = self
            .status
            .selections
            .iter()
            .filter_map(|&idx| self.status.entries.get(idx))
            .filter_map(|entry| entry.file.to_str())
            .collect();
        args.extend(file_args);
        let command_result = self.raw_command(&args);
        self.refresh_svn_status();
        match command_result {
            Ok(_) => Ok(true),
            Err(e) => Err(format!("Error en el commit: {}", e)),
        }
    }

    pub fn add_to_svn(&mut self, idx: usize) {
        let mut args = vec!["add"];
        if let Some(entry) = self.status.entries.get(idx) {
            if let Some(file) = entry.file.to_str() {
                args.push(file);
            }
        }
        let _ = self.raw_command(&args);
        self.refresh_svn_status();
    }

    pub fn revert_to_svn(&mut self, idx: usize) {
        let mut args = vec!["revert"];
        if let Some(entry) = self.status.entries.get(idx) {
            if let Some(file) = entry.file.to_str() {
                args.push(file);
            }
        }
        let _ = self.raw_command(&args);
        self.refresh_svn_status();
    }
}

impl Default for SvnClient {
    fn default() -> Self {
        SvnClient::new(".")
    }
}

pub fn style_for_status(state: &str) -> Style {
    match state {
        "M" => Style::new().fg(Color::Blue),         // Modified
        "A" => Style::new().fg(Color::Green),        // Added
        "D" => Style::new().fg(Color::Red),          // Deleted
        "C" => Style::new().fg(Color::LightRed),     // Conflict
        "?" => Style::new().fg(Color::Yellow),       // Untracked
        "!" => Style::new().fg(Color::LightRed),     // Missing
        "I" => Style::new().fg(Color::DarkGray),     // Ignored
        "R" => Style::new().fg(Color::Cyan),         // Replaced
        "X" => Style::new().fg(Color::Magenta),      // External
        "~" => Style::new().fg(Color::LightMagenta), // Obstructed
        _ => Style::new(),                           // Default
    }
}
