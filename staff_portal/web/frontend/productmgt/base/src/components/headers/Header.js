import {Link} from 'react-router-dom';

import title_logo from '../../img/title_logo.svg';

const Header = (props) => {
    return (
        <>
          <header className="navbar navbar-expand-md navbar-light">
            <div className="container-xl">
                <img src={title_logo} alt="Tabler default logo" className="navbar-brand-image" />
                <Link to='/'  className="navbar-brand navbar-brand-autodark d-none-navbar-horizontal pr-0 pr-md-3">
                    <h3>Product Admin Dashbaord</h3>
                </Link>
                <div className="navbar-nav flex-row order-md-last">
                    <div className="nav-item dropdown d-none d-md-flex mr-3">
                        notification
                    </div>
                    <div className="nav-item dropdown">
                        user profile
                    </div>
                </div>
            </div>
          </header>
        </>
    );
};

export default Header;

