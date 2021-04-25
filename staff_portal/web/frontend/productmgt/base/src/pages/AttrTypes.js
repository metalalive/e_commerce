import {Link} from 'react-router-dom';

const AttrTypes = (props) => {
    return (
        <>
            <p>
              attribute types commonly used among all saleable items
            </p>
            <div className="container-xl">
                <Link className="btn btn-secondary btn-pill btn-block" to='./tags/add'> add new types </Link>
                <Link className="App-link" to='/'> edit </Link>
                <Link className="App-link" to='/'> delete </Link>
            </div>
        </>
    );
};

export default AttrTypes;

