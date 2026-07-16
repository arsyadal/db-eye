# DB-Eye Brand Kit & Visual Identity

This document defines the visual design system, typography, color palette, and layout principles for **DB-Eye**. 

A vector version of the presentation board and logo mark is available at the root: [logo.svg](../logo.svg).

---

## 1. Visual DNA & Metaphor

DB-Eye is a professional, keyboard-driven Terminal UI (TUI) database browser. Its branding is **clean, precise, safety-first, and terminal-native**.

```
    Database Structure          Focus/Inspection           Active Operation
  [Isometric Cylinders]   +   [Focus Lens Eye Path]   +  [Terminal Cursor Block]
```

- **The Cylinders**: Represent the storage engine, data layers, and schemas.
- **The Eye**: Represents scanning, filtering, and real-time monitoring.
- **The Cursor**: Represents input, control, and Vim-style navigation.

---

## 2. Color Palette

DB-Eye uses a monochrome base with a single vibrant accent to ensure maximum readability in all terminal color schemes.

### Primary Colors (UI Structure)
| Color | Hex | ANSI Code | Role |
| :--- | :--- | :--- | :--- |
| **Deep Charcoal** | `#0C0D0E` | `30` (Black) | Main background / canvas |
| **Warm White** | `#FFFFFF` | `37` (White) | Primary text, headers, active selection text |
| **Slate Gray** | `#666666` | `90` (Bright Black) | Border borders, inactive tabs, helper text |

### Accent Colors (Status & Alerts)
| Color | Hex | ANSI Code | Role |
| :--- | :--- | :--- | :--- |
| **Terminal Green** | `#00FF66` | `32` / `92` (Green) | Active cursor, safe read operations, success states |
| **Warning Amber** | `#FFBD2E` | `33` / `93` (Yellow) | Inline edit prompts, connection warnings |
| **Danger Red** | `#FF5F56` | `31` / `91` (Red) | Errors, delete confirmation prompts, destructive queries |

---

## 3. Typography

All typography must be monospaced and align perfectly with standard terminal grids.

- **Primary Interface**: Monospaced font family (e.g., `SF Mono`, `Fira Code`, `JetBrains Mono`).
- **Logo Wordmark**: `Courier New` or custom geometric slab.
- **Text Alignment**: Strictly grid-based (using double-borders `║` / `═` or single-line borders `│` / `─` to separate views).

---

## 4. UI Grid & Layout (TUI Blueprint)

To preserve the clean aesthetic, DB-Eye's UI splits into three distinct zones:

```
┌─────────────────────────────────────────────────────────────┐
│ DB-EYE: mydb.sqlite [Read-Only]                [Tab 1] [x]  │ <- Header (Warm White)
├──────────────────────┬──────────────────────────────────────┤
│ 📁 Tables            │ 🔍 filter: users                     │
│ ───────────────────  │ ──────────────────────────────────── │
│ ▸ users              │  id  │ name     │ email              │ <- Table Grid
│   orders             │ ─────┼──────────┼─────────────────── │
│   products           │  1   │ alice    │ alice@domain.com   │
│   logs               │  2   │ bob      │ bob@domain.com     │
│                      │      │          │                    │
│                      │      │          │                    │
│                      │      │          │                    │
└──────────────────────┴──────┴──────────┴────────────────────┘
│ [Enter] Select  [?] Help  [Ctrl+C] Quit     Query run: 2.4ms│ <- Footer / Status (Slate Gray)
└─────────────────────────────────────────────────────────────┘
```

---

## 5. ASCII Art (Terminal Greeting)

When launched without arguments, or when loading, DB-Eye displays this high-contrast ASCII header:

```text
    ___     ___        ______                 
   /   \   /   \      / ____/___  __  ______  
  /  /\ \ / /\  \    / __/ / __ \/ / / / __ \ 
 /  /_/ // /_/  /   / /___/ /_/ / /_/ /  ___/ 
/______//______/   /_____/\__, /\__, /\____/  
                         /____//____/         
```
