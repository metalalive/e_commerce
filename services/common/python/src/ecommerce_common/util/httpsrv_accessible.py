import logging
import http.client
import sys
import socket
import argparse


def main():
    parser = argparse.ArgumentParser(description="Check FastAPI/Uvicorn server health.")
    parser.add_argument(
        "--timeout-secs", type=int, required=True, help="timeout in seconds for connection check."
    )
    parser.add_argument(
        "--port", type=int, required=True, help="Port number of the server to check."
    )
    parser.add_argument(
        "--host", type=str, required=True, help="domain host of the server to check."
    )
    parser.add_argument(
        "--uri-path",
        type=str,
        required=True,
        help="URI path to the specified domain host with --host.",
    )
    args = parser.parse_args()

    timeout = args.timeout_secs
    port = args.port
    HOST = args.host
    PATH = args.uri_path

    try:
        conn = http.client.HTTPConnection(HOST, port, timeout=timeout)
        conn.request("GET", PATH)
        response = conn.getresponse()

        logging.info(
            f"Server at {HOST}:{port} responded with status {response.status} ({response.reason}). Ready."
        )
        sys.exit(0)

    except ConnectionRefusedError:
        logging.error(f"Connection to {HOST}:{port} refused ([Errno 111]). Server not ready yet.")
        sys.exit(1)
    except (socket.timeout,) as e:
        logging.error(
            f"Server at {HOST}:{port} is not reachable or responsive ({type(e).__name__}: {e}). Not ready."
        )
        sys.exit(1)
    except Exception as e:
        logging.error(
            f"An unexpected error occurred during health check: {type(e).__name__}: {e}. Not ready."
        )
        sys.exit(1)
    finally:
        if "conn" in locals() and conn:
            conn.close()


if __name__ == "__main__":
    root_logger = logging.getLogger()
    if root_logger.handlers:
        for handler in root_logger.handlers:
            root_logger.removeHandler(handler)

    logging.basicConfig(
        level=logging.DEBUG,
        format="%(asctime)s - %(levelname)s - %(message)s",
        stream=sys.stdout,
    )
    main()
