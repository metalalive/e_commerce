import * as toolkit from "/static/js/toolkit.js";

var _data_source = {
    path: '/static/location/data/',
    country:{filename:'country_code.csv', data:null, select_list: null, preload:null,} ,
    locality:{}, // e.g. 'country_code' : {filename: null, data: null, select_list: null}
};


class LocationForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',    type:'hidden', value:''}, // id specific to the geo-location address
            {name:'uid',   type:'hidden', value:''}, // id bound with specific user, used only in update scene
            {name:'country' ,     label:'country' ,    type:'', value:''}, // single-choice search box
            {name:'locality',     label:'locality',    type:'', value:''}, // single-choice search box
            {name:'province',     label:'province',    type:'hidden', value:''},
            {name:'street',       label:'street',      type:'text', value:''},
            {name:'detail',       label:'detail',      type:'text', value:''},
            {name:'floor',        label:'floor',       type:'number', value:'1'},
            {name:'description',  label:'description', type:'text', value:''},
        ];
    }

    _new_single_form(init_form_data)
    {
        var _components = [];
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);

        var element_province = React.createElement('input', _fields['province']); // state/province/county
        _fields['locality'].last_lvl_elm = element_province;
        var element_locality = React.createElement(toolkit.TagInstantSearchBox, _fields['locality']);
        _fields['country'].nxt_lvl_elm = element_locality;
        var element_country  = React.createElement(toolkit.TagInstantSearchBox, _fields['country']);

        _components.push(
            React.createElement('input', _fields['id']),
            React.createElement('input', _fields['uid']),
            element_province,
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, 'Country'),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, 'City'),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, 'Street'),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',
                    children:toolkit.render_form_window_btns(this)},
                ),
            ),
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, element_country
                ),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, element_locality
                ),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',},
                    React.createElement('input', _fields['street'])
                ),
            ),
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-9',}, 'Detail'),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',}, 'Floor'),
            ),
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-9',},
                    React.createElement('input', _fields['detail'])
                ),
                React.createElement("div", {key: this.get_unique_key(), className:'col-3',},
                    React.createElement('input', _fields['floor'])
                ),
            ),
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-12',}, 'Description'),
            ),
            React.createElement("div", {key: this.get_unique_key(), className:'row',},
                React.createElement("div", {key: this.get_unique_key(), className:'col-12',},
                    React.createElement('input', _fields['description'])
                ),
            ),
        );
        var row = React.createElement("div", {key: this.get_unique_key(), className:'card bg-light p-1',children:_components}, )
        return [row];
    } // end of _new_single_form
} // end of class LocationForm



function get_default_form_props(dom_ref)
{
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    var out = toolkit.get_default_form_props(dom_ref, append_new_form);
    out.btns.addform.className   = "btn btn-sm btn-outline-secondary p-1";
    out.btns.closeform.className = "btn btn-sm btn-outline-secondary p-1";
    out.form['id']  = {defaultValue: '',};
    out.form['uid'] = {defaultValue: '',};
    out.form['country' ] = {placeholder:'country', defaultValue:'', whitelist:_data_source.country.select_list,
                            className:'tagify--custom-dropdown',  mode: 'select', evt_cb_add: load_locality_list,
                            extract_prop_on_submit:'value',  evt_cb_remove: unload_locality_list, dropdown: dropdown,};
    out.form['locality'] = {placeholder:'city/town', defaultValue:'', whitelist:[], extract_prop_on_submit:'value',
                            className:'tagify--custom-dropdown',  mode: 'select', dropdown: dropdown,
                            evt_cb_add: update_province_field };
    out.form['province'] = {defaultValue:''};
    out.form['street'  ] = {className:"form-control", placeholder:'', defaultValue:''};
    out.form['detail'  ] = {className:"form-control", placeholder:'e.g. village, building ... etc', defaultValue:''};
    out.form['floor'   ] = {className:"form-control", tilte:'1,2,3 ... , note B1 is -1, B2 is -2 ... etc', defaultValue:'1'};
    out.form['description'] = {className:"form-control", placeholder:'extra information about this location', defaultValue:''};
    return out;
}


function append_new_form(dom_ref, props) {
    if(props == null){
        props = [];
    } else if (!(props instanceof Array)) {
        throw "initial data to render quota subforms must be a list of property objects"
    }
    if(props.length == 0) {
        props[0] = get_default_form_props(dom_ref);
    }
    var container = (dom_ref instanceof HTMLElement) ? dom_ref: dom_ref.current;
    for(var idx = 0; idx < props.length; idx++) {
        LocationForm.add_form(container, props[idx]);
    }
} // end of append_new_form

 



function  location_report_load_error(http_resp, args){
    console.log("[XMLHttpRequest] response error, readyState :"+ http_resp.readyState +", status: "+ http_resp.status);
}

function  parse_loaded_country_data(http_resp, args){
    var json_obj  = toolkit.convert_csv_to_json(http_resp.responseText);
    _data_source.country.data = json_obj;
    _data_source.country.select_list = json_obj.map(x => x.code);
    toolkit.api_data_ready_cb(null, args);
}

function  init_parse_loaded_locality(http_resp, args) {
    _parse_loaded_locality_data(http_resp, args);
    toolkit.api_data_ready_cb(null, args);
}

function  _parse_loaded_locality_data(http_resp, args) {
    var country_code = args.country_code;
    var json_obj  = toolkit.convert_csv_to_json(http_resp.responseText);
    _data_source.locality[country_code] = {filename: args.csv_full_path, data: null, select_list: null};
    _data_source.locality[country_code].data = json_obj;
    _data_source.locality[country_code].select_list = json_obj.map(x => {return {value:x.city_ascii, admin_name:x.admin_name}});
}


function  onclick_parse_loaded_locality(http_resp, args) {
    var country_code = args.country_code;
    _parse_loaded_locality_data(http_resp, args);
    args.country_comp.update_nxt_lvl_comp(_data_source.locality[country_code].select_list);
    console.log("locality data for the country (code:"+ country_code +") loaded successfully.");
}


function _load_locality_list(kwargs) {
    var uripath = kwargs.uripath;
    var country_code = kwargs.country_code;
    var callbacks = kwargs.callbacks;
    var cb_args = kwargs.cb_args;

    var csv_full_path = uripath + country_code +".csv";
    var req = {method:'GET', file_path: csv_full_path, callback:{}, cb_args:{},};
    req.callback.failure = callbacks.failure ;
    req.callback.succeed = callbacks.succeed ;
    req.cb_args.succeed = {csv_full_path: csv_full_path, country_code: country_code, ... cb_args.succeed};
    toolkit.load_file_http(req);
}


function load_locality_list(evt) {
    var new_tag = evt.detail.value.data;
    var country_code = new_tag.value.toLowerCase();
    var country_comp = evt.detail.tagify.react_comp;
    if(_data_source.locality[country_code] != undefined) {
        console.log("locality data for the country (code:"+ country_code +") was already loaded.");
        country_comp.update_nxt_lvl_comp(_data_source.locality[country_code].select_list);
    } else {
        var kwargs = {uripath: _data_source.path, country_code: country_code,
            callbacks: {failure: location_report_load_error, succeed: onclick_parse_loaded_locality},
            cb_args : {succeed: {country_comp: country_comp}},
        };
        _load_locality_list(kwargs);
    }
}


function unload_locality_list(evt) {
    var new_tag = evt.detail.value.data;
    var country_code =  new_tag.value.toLowerCase();
    console.log("[unload_locality_list] country_code : ("+ country_code +")");
    var country_comp = evt.detail.tagify.react_comp;
    country_comp.update_nxt_lvl_comp([]);
}


function update_province_field(evt) {
    var new_tag = evt.detail.value.data;
    var locality = new_tag.value;
    var province = new_tag.admin_name;
    console.log("[update_province_field]  : ("+ province +","+ locality +")");
    var province_comp = evt.detail.tagify.react_comp.props.last_lvl_elm.ref.current;
    var province_dom = ReactDOM.findDOMNode(province_comp);
    province_dom.value = province;
}


function load_country_data(form_layout, edit_data) {
    // any other application, which invokes this location application, has to initialize country-code
    // search list by calling this function.
    var req = {method:'GET', file_path:_data_source.path + _data_source.country.filename,
                callback:{}, cb_args:{}, };
    req.callback.failure = location_report_load_error;
    req.callback.succeed = parse_loaded_country_data;
    req.cb_args.succeed = {caller: form_layout};
    toolkit.load_file_http(req);
    if(edit_data) { // optionally load locality data for edit data
        var preload = edit_data.map((x) => x.locations);
        preload = preload.map((x) => { return x.map(y => y.country.toLowerCase()); });
        preload = [].concat.apply([], preload); // flatten from 2D to 1D
        preload = [... new Set(preload)]; // remove duplicates
        _data_source.country.preload = preload;
        preload.map(x => {
            var kwargs = {uripath: _data_source.path, country_code: x,
                callbacks: {failure: location_report_load_error, succeed: init_parse_loaded_locality},
                cb_args : {succeed: {caller: form_layout},},
            };
            _load_locality_list(kwargs);
        });
    }
}


function init_api_data_done()
{
    var out = _data_source.country.data ? true : false;
    var preload = _data_source.country.preload;
    if(preload) {
        var exist = preload.map(x => (_data_source.locality[x] ? true: false));
        exist.push(out);
        out = exist.reduce((a,n) => {return a && n;});
        // console.log("locationform.init_api_data_done, exist: "+ exist +" , out:"+ out);
    }
    return out;
}


function render_with_data(data) {
    var out = [];
    for (var jdx = 0; jdx < data.length; jdx++) {
        var prop = get_default_form_props(null);
        prop.form['uid'].defaultValue  = data[jdx].uid;
        prop.form['id'].defaultValue   = data[jdx].id;
        prop.form['province'].defaultValue = data[jdx].province;
        prop.form['street'].defaultValue = data[jdx].street;
        prop.form['detail'].defaultValue = data[jdx].detail;
        prop.form['floor'].defaultValue = data[jdx].floor;
        prop.form['description'].defaultValue = data[jdx].description;
        prop.form['country'].defaultValue = data[jdx].country;
        prop.form['locality'].defaultValue = data[jdx].locality;
        prop.form['locality'].whitelist = _data_source.locality[
            data[jdx].country.toLowerCase()
        ].select_list ;
        out.push(prop);
    } // end of inner loop
    return out;
}


export {render_with_data, get_default_form_props, append_new_form, load_country_data, init_api_data_done,};


