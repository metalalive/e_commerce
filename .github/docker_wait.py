#!/usr/bin/env python3
import subprocess
import sys
import argparse
import time


def get_container_status(container_id):
    """
    Extract current status of a docker container using docker inspect command.

    Args:
        container_id (str): Container ID or name

    Returns:
        str: Container status ('created', 'running', or 'exited')

    Raises:
        subprocess.CalledProcessError: If docker command fails
        KeyError: If status information is not found in docker inspect output
    """
    try:
        # Run docker inspect command and capture output
        cmd = ["docker", "inspect", "--format={{.State.Status}}", container_id]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        # Strip any whitespace or newlines from the output
        status = result.stdout.strip()

        # Validate the status matches expected values
        if status not in ["created", "running", "exited"]:
            raise ValueError(f"Unexpected container status: {status}")

        return status

    except subprocess.CalledProcessError as e:
        print(f"Error executing docker command: {e}", file=sys.stderr)
        print(f"stderr: {e.stderr}", file=sys.stderr)
        raise
    except Exception as e:
        print(f"Error getting container status: {e}", file=sys.stderr)
        raise


def main():
    """
    Main function to wait for a docker container to reach a target status.
    """
    parser = argparse.ArgumentParser(
        description="Wait for a Docker container to reach a target status."
    )
    parser.add_argument("--container-name", required=True, help="Container ID or name.")
    parser.add_argument(
        "--target-status",
        required=True,
        choices=["created", "running", "exited"],
        help="The target status to wait for.",
    )
    parser.add_argument("--timeout-secs", type=int, required=True, help="Timeout in seconds.")

    args = parser.parse_args()
    container_id = args.container_name
    target_status = args.target_status
    timeout = args.timeout_secs

    interval = 7  # seconds
    start_time = time.time()

    while time.time() - start_time < timeout:
        try:
            status = get_container_status(container_id)
            print(f"Container '{container_id}' status: {status}. Waiting for '{target_status}'.")
            if status == target_status:
                print(f"Container '{container_id}' reached target status '{target_status}'.")
                sys.exit(0)
        except Exception:
            sys.exit(1)

        time.sleep(interval)

    print(
        f"Timeout waiting for container '{container_id}' to reach status '{target_status}'.",
        file=sys.stderr,
    )
    sys.exit(1)


if __name__ == "__main__":
    main()
