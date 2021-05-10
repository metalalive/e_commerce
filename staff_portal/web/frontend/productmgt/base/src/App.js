import {Route, Switch, Link} from 'react-router-dom';

import  Header    from './components/headers/Header';
import  Menubar   from './components/headers/Menubar';
import  Dashboard from './pages/Dashboard';
import  Tags      from './pages/Tags';
import  AttrTypes from './pages/AttrTypes';
import  Ingredients      from './pages/Ingredients';
import  SaleableItems    from './pages/SaleableItems';
import  SaleablePackages from './pages/SaleablePackages';
import  ProjDev   from './pages/ProjDev';
import  {patch_string_prototype} from './js/common/native.js';

import './css/tabler.min.css';
import './css/App.css';
import './js/bootstrap.bundle.min.js';

function App() {
  patch_string_prototype();
  return (
    <>
        <div className="App">
            <Route path='/' component={Header} />
            <Route path='/' component={Menubar} />
            <Switch>
                <Route path='/tags'           component={Tags} />
                <Route path='/attr_types'     component={AttrTypes} />
                <Route path='/ingredients'    component={Ingredients} />
                <Route path='/saleable_items' component={SaleableItems} />
                <Route path='/saleable_pkgs'  component={SaleablePackages} />
                <Route path='/proj_dev'       component={ProjDev} />
                <Route path='/'     component={Dashboard} />
            </Switch>
        </div>
    </>
  );
}

export default App;

