# DCL Language Support for VS Code

This extension provides syntax highlighting and language configuration support for the **Differentiable Cryptographic Language (DCL)** (`.dcl`) in Visual Studio Code.

## Features

- **Syntax Highlighting**: Beautiful colorization for DCL keywords, types, boolean constants, big integer literals, operators (`&&`, `||`, `!`, `!=`, comparisons), and line comments.
- **Language Configurations**: Auto-closing bracket pairs, bracket matching, and standard line-comment settings (`//`).

## How to Install and Run Locally

Since this extension is contained within the DCL workspace, you can easily load it into VS Code:

### Option 1: Symlink into extensions directory (Recommended)
Run the following command in your terminal to link the extension to your local VS Code extensions directory:
```bash
ln -s "/Users/liuyukai/CREATE/auv/editors/vscode" ~/.vscode/extensions/dcl-vscode
```
Then restart VS Code or run the `Developer: Reload Window` command.

### Option 2: Copy the folder
Alternatively, copy the `vscode` directory directly to VS Code extensions:
```bash
cp -r "/Users/liuyukai/CREATE/auv/editors/vscode" ~/.vscode/extensions/dcl-vscode
```

### Option 3: Extension Development Host
1. Open the `/Users/liuyukai/CREATE/auv/editors/vscode` folder in VS Code.
2. Press `F5` to start debugging. This will launch a new window (Extension Development Host) with the DCL language support extension automatically enabled.
3. Open any `.dcl` file in this window to test syntax highlighting.
