import {Link} from 'react-router-dom';


// horizontal menubar
const Menubar = (props) => {
    function _change_button_status(e)
    {
        var target = e.nativeEvent.target;
        while (target.tagName !== 'A') {
            target = target.parentNode;
        }
        var active_navitem = target.parentNode;
        var navlist = active_navitem.parentNode.querySelectorAll("li");
        for(var idx = 0; idx < navlist.length; idx++) {
            var navitem = navlist[idx];
            if(active_navitem === navitem) {
                active_navitem.classList.add('active');
            } else {
                navitem.classList.remove('active');
            }
        }
    }

    return (
        <>
          <div className="navbar-expand-md">
            <div className="collapse navbar-collapse" id="navbar-menu">
              <div className="navbar navbar-light">
                <div className="container-xl">
                  <ul className="navbar-nav">
                    <li className="nav-item">
                      <Link className="nav-link" onClick={_change_button_status} to="/tags" >
                        <span className="nav-link-title"> Tags </span>
                      </Link>
                    </li>
                    <li className="nav-item">
                      <Link className="nav-link" onClick={_change_button_status} to="/attr_types" >
                        <span className="nav-link-title"> attribute types </span>
                      </Link>
                    </li>
                    <li className="nav-item">
                      <Link className="nav-link" onClick={_change_button_status} to="/saleable_items" >
                        <span className="nav-link-title"> saleable items </span>
                      </Link>
                    </li>
                    <li className="nav-item">
                      <Link className="nav-link" onClick={_change_button_status} to="/saleable_pkgs" >
                        <span className="nav-link-title"> saleable packages </span>
                      </Link>
                    </li>
                    <li className="nav-item">
                      <Link className="nav-link" onClick={_change_button_status} to="/proj_dev" >
                        <span className="nav-link-title"> project development </span>
                      </Link>
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </>
    );
};

export default Menubar;

