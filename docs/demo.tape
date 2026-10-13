Output docs/demo.gif
Output docs/demo.mp4

Set Shell "bash"
Set FontSize 15
Set Width 920
Set Height 540
Set Padding 20
Set TypingSpeed 55ms
Set WindowBar "Colorful"
Set BorderRadius 10

Set Theme {
  "background": "#0D1117",
  "foreground": "#E6EDF3",
  "black": "#0D1117",
  "brightBlack": "#6E7681",
  "red": "#FF7B72",
  "brightRed": "#FFA198",
  "green": "#3FB950",
  "brightGreen": "#56D364",
  "yellow": "#D29922",
  "brightYellow": "#E3B341",
  "blue": "#58A6FF",
  "brightBlue": "#79C0FF",
  "magenta": "#BC8CFF",
  "brightMagenta": "#D2A8FF",
  "cyan": "#39C5BB",
  "brightCyan": "#56D4DD",
  "white": "#E6EDF3",
  "brightWhite": "#FFFFFF",
  "cursor": "#E6EDF3"
}

Hide
Type "clear && ./target/release/db-eye demo.db"
Enter
Sleep 1s
Show

Sleep 1.2s

# 1. Browse tables with j/k
Down 1
Sleep 400ms
Down 1
Sleep 500ms

# 2. Open products table
Enter
Sleep 1s

# 3. Scroll rows and sort by column
Down 2
Sleep 300ms
Down 2
Sleep 400ms
Type "o"
Sleep 700ms
Type "o"
Sleep 700ms

# 4. Search and filter rows
Type "/"
Sleep 200ms
Type "Hard"
Sleep 800ms
Escape
Sleep 500ms

# 5. Table stats popup
Type "s"
Sleep 1.2s
Escape
Sleep 400ms

# 6. SQL Query Editor with Live Tab Autocomplete
Type ":"
Sleep 400ms
Type "sel"
Sleep 500ms
Tab
Sleep 400ms
Type "* from p"
Sleep 600ms
Tab
Sleep 500ms
Type "whe"
Sleep 400ms
Tab
Sleep 300ms
Type "price > 100"
Sleep 800ms
Enter
Sleep 2s

# Clean exit
Ctrl+C
Sleep 500ms
