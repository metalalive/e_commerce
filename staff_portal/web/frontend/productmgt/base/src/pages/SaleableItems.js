import {Link} from 'react-router-dom';

const SaleableItems = (props) => {
    return (
        <>
            <p>
              all saleable items, attributes applied to them,
              the media files to describe them, price estimate,
              and price history of each item ...
            </p>
            <div className="container-xl">
                <Link className="btn btn-info btn-block" to='/tags/add'> add new items </Link>
                <Link className="App-link" to='/'> edit </Link>
                <Link className="App-link" to='/'> delete </Link>
            </div>
        </>
    );
};

export default SaleableItems;

