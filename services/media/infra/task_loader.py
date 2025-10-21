import sys
import inspect

from infra.renew_certs import *  # noqa: F403
from infra.render_template import *  # noqa: F403

if __name__ == "__main__":
    # The following code block has been updated to search for classes
    # from the imported module (filechunk.py) as well as the current module.
    # This change ensures that the program can find the moved classes.
    # We collect classes from both places to allow other classes that
    # might still reside in task_loader.py to be found.

    all_cls_members = inspect.getmembers(sys.modules[__name__], inspect.isclass)

    target_class_name = sys.argv[1]
    target_class = None
    for name, cls in all_cls_members:
        if name == target_class_name:
            target_class = cls
            break

    assert target_class, 'invalid class name "%s" received \n' % target_class_name
    argv = sys.argv[2:]
    target_class().start(argv)
