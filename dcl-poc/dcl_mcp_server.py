import sys
import json
import subprocess
import tempfile
import os

def log_error(msg):
    sys.stderr.write(f"[DCL-MCP-LOG]: {msg}\n")
    sys.stderr.flush()

def handle_initialize(req_id, params):
    response = {
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "DCL-MCP-Server",
                "version": "0.1.0"
            }
        }
    }
    return response

def handle_tools_list(req_id):
    tools = [
        {
            "name": "dcl_get_syntax",
            "description": "Get the formal grammar, keyword list, and syntax rules for DCL.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "dcl_check_code",
            "description": "Check a DCL source code snippet for syntax and type errors.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "DCL source code text to verify."
                    }
                },
                "required": ["code"]
            }
        },
        {
            "name": "dcl_compile_circuit",
            "description": "Compile a DCL file to Circom or ACIR circuit code.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input_file": {
                        "type": "string",
                        "description": "Absolute path to the input DCL file."
                    },
                    "output_file": {
                        "type": "string",
                        "description": "Absolute path to the output destination file."
                    },
                    "backend": {
                        "type": "string",
                        "description": "Target backend (e.g. 'circom' or 'acir'). Default is 'circom'."
                    }
                },
                "required": ["input_file", "output_file"]
            }
        }
    ]
    return {
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "tools": tools
        }
    }

def handle_tools_call(req_id, params):
    tool_name = params.get("name")
    arguments = params.get("arguments", {})

    if tool_name == "dcl_get_syntax":
        syntax_text = """DCL (Differentiable Cryptographic Language) Syntax Summary:

1. Visibilities for Circuit Parameters:
   - private: Secret witness input (hidden).
   - public: Shared public input (revealed).
   - shared: MPC shared secret.

2. Primitives:
   - Field: Prime field element (BN254 field).
   - bool: Boolean value (true, false).

3. Variables & Mutability:
   - let x = val; (immutable by default)
   - let mut y = val; y = new_val; (mutable)

4. Control Flow:
   - Bounded Loops: 'for var in start..end { body }' (bounds must be Field constants).
   - Conditional branch: 'if cond { then } else { otherwise }'

5. Standard Library Modules:
   - std::crypto: poseidon(x, y), verify_merkle(leaf, path, root)
   - std::fixed: from_int(x), to_int(x), add(a, b), sub(a, b), mul(a, b), div(a, b), gte(a, b), lte(a, b)
   - std::utils: range_check(value, bits), assert_in_range(x, min, max)"""
        return make_tool_response(req_id, syntax_text)

    elif tool_name == "dcl_check_code":
        code = arguments.get("code", "")
        # Save code snippet to temp file
        fd, path = tempfile.mkstemp(suffix=".dcl")
        try:
            with os.fdopen(fd, 'w') as tmp:
                tmp.write(code)
            
            # Execute compiler check command
            dcl_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "../dcl"))
            res = subprocess.run(
                ["cargo", "run", "--package", "dcl-cli", "--", "check", path],
                cwd=dcl_dir,
                capture_output=True,
                text=True
            )
            if res.returncode == 0:
                output = "Check successful! No errors found.\n" + res.stdout
            else:
                output = f"Errors found:\n{res.stderr}\n{res.stdout}"
            return make_tool_response(req_id, output)
        finally:
            os.remove(path)

    elif tool_name == "dcl_compile_circuit":
        input_file = arguments.get("input_file")
        output_file = arguments.get("output_file")
        backend = arguments.get("backend", "circom")

        dcl_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "../dcl"))
        res = subprocess.run(
            ["cargo", "run", "--package", "dcl-cli", "--", "compile", input_file, "-o", output_file, "-b", backend],
            cwd=dcl_dir,
            capture_output=True,
            text=True
        )
        if res.returncode == 0:
            output = f"Compilation successful! Circuit written to {output_file}\n" + res.stdout
        else:
            output = f"Compilation failed:\n{res.stderr}\n{res.stdout}"
        return make_tool_response(req_id, output)

    else:
        return {
            "jsonrpc": "2.0",
            "id": req_id,
            "error": {
                "code": -32601,
                "message": f"Method not found: {tool_name}"
            }
        }

def make_tool_response(req_id, text):
    return {
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "content": [
                {
                    "type": "text",
                    "text": text
                }
            ]
        }
    }

def main():
    log_error("DCL MCP Server started.")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            req_id = req.get("id")
            method = req.get("method")
            params = req.get("params", {})

            log_error(f"Received request: {method}")

            if method == "initialize":
                res = handle_initialize(req_id, params)
            elif method == "tools/list":
                res = handle_tools_list(req_id)
            elif method == "tools/call":
                res = handle_tools_call(req_id, params)
            else:
                res = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": {
                        "code": -32601,
                        "message": f"Method not found: {method}"
                    }
                }
            
            # Write JSON-RPC response to stdout
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()
        except Exception as e:
            log_error(f"Exception: {str(e)}")

if __name__ == "__main__":
    main()
