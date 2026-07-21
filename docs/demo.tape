Output docs/demo.gif

Set Shell "bash"
Set FontSize 16
Set FontFamily "JetBrains Mono"
Set Width 1200
Set Height 700
Set Padding 20
Set TypingSpeed 60ms

Set Theme {
  "background": "#0C0D0E",
  "foreground": "#FFFFFF",
  "black": "#0C0D0E",
  "brightBlack": "#666666",
  "green": "#00FF66",
  "brightGreen": "#00FF66",
  "yellow": "#FFBD2E",
  "brightYellow": "#FFBD2E",
  "red": "#FF5F56",
  "brightRed": "#FF5F56",
  "cursor": "#00FF66"
}

Hide
Type "clear && ./target/release/db-eye sample.db"
Enter
Sleep 500ms
Show

Sleep 1s

# Browse tables
Down 1
Sleep 400ms
Down 1
Sleep 400ms
Up 1
Sleep 500ms

# Open employees table
Enter
Sleep 1s

# Scroll rows
Down 3
Sleep 300ms
Down 3
Sleep 800ms

# Search
Type "/"
Sleep 300ms
Type "budi"
Sleep 1s
Escape
Sleep 500ms

# Run a custom SQL query
Type ":"
Sleep 300ms
Type "SELECT name, salary FROM employees ORDER BY salary DESC"
Sleep 500ms
Enter
Sleep 1.5s

# Show help popup
Type "?"
Sleep 1.5s
Type "?"
Sleep 500ms

Ctrl+C
Sleep 500ms
