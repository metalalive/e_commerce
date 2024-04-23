import errno
import shutil
import os
from os.path import abspath, dirname, join, normcase, sep


def _fd(f):
    """get a file descriptor from something which could be a file or fd"""
    return f.fileno() if hasattr(f, "fileno") else f


# Note this project does not support Windows OS
try:
    import fcntl

    LOCK_SH = fcntl.LOCK_SH  # shared lock
    LOCK_EX = fcntl.LOCK_EX  # exclusive lock
    LOCK_NB = fcntl.LOCK_NB  # non-blocking
except (ImportError, AttributeError) as e:
    # file locking is NOT supported
    LOCK_SH = LOCK_EX = LOCK_NB = 0

    # dummy functions which don't do anything
    def fd_lock(f, flags):
        return False  # always failed to lock

    def fd_unlock(f):
        return True

else:

    def fd_lock(f, flags):
        ret = fcntl.flock(_fd(f), flags)
        return ret == 0

    def fd_unlock(f):
        ret = fcntl.flock(_fd(f), fcntl.LOCK_UN)
        return ret == 0


def safe_path_join(base, *paths):
    """
    Join one or more path components to the base path component intelligently.
    Return a normalized, absolute version of the final path.
    Raise ValueError if the final path isn't located inside of the base path
    component.
    """
    final_path = abspath(join(base, *paths))
    base_path = abspath(base)
    # ensure final_path starts with base_path. (using normcase so we
    # don't false-negative on case insensitive operating system i.e. Windows)
    # This is to prevent relative parant path in argument `paths`, such as
    # safe_path_join("/path/to/dir", "..")
    # safe_path_join("/path/to/dir", "../../disallowed_path")
    # safe_path_join("/path/to/dir", "/other/disallowed/path")
    final_path_norm = normcase(final_path)
    base_path_norm = normcase(base_path)
    outside_base = not final_path_norm.startswith(base_path_norm + sep)
    # both of the path may be exactly the same  if `paths` is empty
    paths_different = final_path_norm != base_path_norm
    #### TODO, figure out why Django applies the condition below
    #### (1) what if user application really needs root path of OS as root path of its media storage ?
    ##not_root_path = dirname(base_path_norm) != base_path_norm
    if outside_base and paths_different:
        msg_pattern = (
            "The joined path ({}) is located outside of the base path component ({})"
        )
        raise ValueError(msg_pattern.format(final_path, base_path))
    return final_path


def _same_file(src, dst):
    return normcase(abspath(src)) == normcase(abspath(dst))


def safe_file_move(old_name, new_name, chunk_size=1024 * 4, allow_overwrite=False):
    """
    Move a file from one location to another in the safest way possible.
    """
    if _same_file(old_name, new_name):
        return
    try:  # try ``os.rename``, which is simple but would break across filesystems.
        if not allow_overwrite and os.access(new_name, os.F_OK):
            raise FileExistsError(
                "Destination file %s exists and allow_overwrite is False." % new_name
            )
        os.rename(old_name, new_name)
        return
    except OSError as e:
        # OSError happens with os.rename() if moving to another filesystem or
        # when moving opened files on certain operating systems.
        pass

    # If os.rename() fails, stream manually from one file to another in pure Python.
    # If the destination file exists and ``allow_overwrite`` is ``False``, raise
    # ``FileExistsError``.
    with open(old_name, "rb") as old_f:
        new_fd_flgs = (
            os.O_WRONLY
            | os.O_CREAT
            | getattr(os, "O_BINARY", 0)
            | (os.O_EXCL if not allow_overwrite else 0)
        )
        # create OS-level file descriptor, for manually locking the file
        new_fd = os.open(new_name, new_fd_flgs)
        try:
            # If the destination file already exists, there may be other processes
            # accessing the same file at the time this function is transferring data
            # bytes from the old file. To handle concurrency issue, it is safe to
            # acquire the file lock before streaming , in order to prevent data
            # corruption if any other thread holds read lock (shared lock) but retrieves
            # unfinished written data.
            fd_lock(new_fd, LOCK_EX)
            current_chunk = None
            while current_chunk != b"":
                current_chunk = old_f.read(chunk_size)
                os.write(new_fd, current_chunk)
        finally:
            fd_unlock(new_fd)
            os.close(new_fd)

    try:
        shutil.copystat(old_name, new_name)
    except PermissionError as e:
        # Certain filesystems (e.g. CIFS) fail to copy the file's metadata if
        # the type of the destination filesystem isn't the same as the source
        # filesystem; ignore that.
        if e.errno != errno.EPERM:
            raise

    os.remove(old_name)


## end of safe_file_move()
