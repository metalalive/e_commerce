import {Link} from 'react-router-dom';

const ProjDev = (props) => {
    return (
        <>
            <p>
              Project Development page, integrated with Trello, a Kanban-style PM tool
            </p>
            <div className="container-xl">
                <Link className="App-link" to='/'> should be a link to external Trello page </Link>
            </div>
        </>
    );
};

export default ProjDev;

