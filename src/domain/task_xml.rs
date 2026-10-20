//! The Start with Windows scheduled task as Task Scheduler XML (research R7; contracts/ui.md).

/// Task XML that starts `exe_path` elevated (highest available) when `user_id` signs in, with no
/// battery restrictions, no time limit and no second instance. The caller writes it as UTF-16 LE
/// with a BOM, matching the declared encoding.
pub fn task_xml(exe_path: &str, user_id: &str) -> String {
    let exe = escape(exe_path);
    let user = escape(user_id);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts Computer Stats when you sign in.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml() -> String {
        task_xml(r"C:\Apps\computer-stats.exe", r"PC\Diuli")
    }

    #[test]
    fn declares_utf16() {
        assert!(xml().starts_with(r#"<?xml version="1.0" encoding="UTF-16"?>"#));
    }

    #[test]
    fn has_every_required_setting() {
        let x = xml();
        for element in [
            "<LogonTrigger>",
            r"<UserId>PC\Diuli</UserId>",
            "<RunLevel>HighestAvailable</RunLevel>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
            r"<Command>C:\Apps\computer-stats.exe</Command>",
        ] {
            assert!(x.contains(element), "missing {element}");
        }
        assert!(!x.contains("<Arguments>"));
    }

    #[test]
    fn escapes_path_and_user() {
        let x = task_xml(r#"C:\A&B <x> "q".exe"#, "PC\\O'Neil");
        assert!(x.contains(r"<Command>C:\A&amp;B &lt;x&gt; &quot;q&quot;.exe</Command>"));
        assert!(x.contains(r"<UserId>PC\O&apos;Neil</UserId>"));
    }
}
