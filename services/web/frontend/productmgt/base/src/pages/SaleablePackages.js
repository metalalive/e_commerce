import {Link} from 'react-router-dom';

const SaleablePackages = (props) => {
    return (
        <>
            <p>
              all saleable packages, iextra attributes applied only at package level,
              the media files to describe them, price estimate,
              and price history of each package ...
            </p>
            <div className="container-xl">
                <Link className="btn btn-info btn-block" to='/tags/add'> add new packages </Link>
                <Link className="App-link" to='/'> edit </Link>
                <Link className="App-link" to='/'> delete </Link>
            </div>
        </>
    );
};

export default SaleablePackages;

