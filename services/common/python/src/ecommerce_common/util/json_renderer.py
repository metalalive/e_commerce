import argparse
import json
import os

# --- Example usage ---
#
# input_template = "template.json"  # JSON template file
# output_file = "output.json"       # Rendered JSON output file
# params = {
#     "user/name": "Alice",
#     "user/age": 30,
#     "user/preferences/theme": "dark",
#     "active": True
# }


def render_json_template(input_template_path, output_file_path, parameters):
    if not os.path.exists(input_template_path):
        raise FileNotFoundError(f"Template file not found: {input_template_path}")

    with open(input_template_path, "r") as f:
        try:
            json_data = json.load(f)
        except json.JSONDecodeError as e:
            raise ValueError(f"Invalid JSON template: {e}")

    def set_nested_value(data, keys, value):
        for key in keys[:-1]:
            # Check if the key is a numeric index (for array access)
            if isinstance(data, list):
                try:
                    idx = int(key)
                    data = data[idx]
                except (ValueError, IndexError) as e:
                    raise KeyError(f"Invalid array index: {key}")
            else:
                # For dictionaries, create the key if it doesn't exist
                data = data.setdefault(key, {})

        # Handle the last key
        final_key = keys[-1]
        if isinstance(data, list):
            try:
                idx = int(final_key)
                data[idx] = value
            except (ValueError, IndexError) as e:
                raise KeyError(f"Invalid array index: {final_key}")
        else:
            data[final_key] = value

    for json_path, value in parameters.items():
        keys = json_path.split("/")
        set_nested_value(json_data, keys, value)

    with open(output_file_path, "w") as f:
        json.dump(json_data, f, indent=4)


def main():
    parser = argparse.ArgumentParser(
        description="Render a JSON template with parameters."
    )
    parser.add_argument(
        "--template", required=True, help="Path to the input JSON template file."
    )
    parser.add_argument(
        "--output", required=True, help="Path to the output rendered JSON file."
    )
    parser.add_argument(
        "--parameters",
        required=True,
        help='Parameters as a single string, e.g., "key1=value1 key2=value2 key3=value3".',
    )
    args = parser.parse_args()
    # Resolve relative paths to absolute paths
    input_template_path = os.path.abspath(args.template)
    output_file_path = os.path.abspath(args.output)

    # Parse parameters into a dictionary
    parameters = {}
    for param in args.parameters.split():
        if "=" not in param:
            raise ValueError(f"Invalid parameter format: {param}. Expected key=value.")
        key, value = param.split("=", 1)
        try:
            # Attempt to parse as JSON for numbers, booleans, etc.
            value = json.loads(value)
        except json.JSONDecodeError:
            pass  # Keep value as a string if not JSON parsable
        parameters[key] = value

    # Call the main function
    render_json_template(input_template_path, output_file_path, parameters)


if __name__ == "__main__":
    main()
