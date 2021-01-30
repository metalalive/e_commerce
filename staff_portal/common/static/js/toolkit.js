
function cvt_list_to_dict(data, key_field_name)
{
    // convert given list of objects to key-based data structure, for ease of search by the given key value
    var out = {};
    for(var idx = 0; idx < data.length; idx++) {
        var item = data[idx];
        var key = item[key_field_name];
        if(!key) {
            throw "the key cannot be null or undefined"
        }
        out[key] = item;
    }
    return out;
}


function truncate_text(text, len_limit)
{
    var len = Math.min(text.length, len_limit);
    var out = text.substr(0, len);
    if(len == len_limit) {
        out += "...";
    }
    return out;
}


// update list of existing DOMs, by writing data from a given object with specific IDs of the DOMs
function update_doms_props(dst_doms, src_data) {
    for(var idx = 0; idx < dst_doms.length; idx++) {
        var dst_dom = dst_doms[idx];
        var custom_data = src_data[dst_dom.id] ;
        for (var key in custom_data) {
            if(key == 'dataset') {
                for (var key2 in custom_data[key]) {
                    dst_dom.dataset[key2] = custom_data.dataset[key2];
                }
            } else if (key == 'onclick' && custom_data[key].length > 1) {
                // TODO, make data type consistent (use array only) on other event types
                for (var jdx = 0; jdx < custom_data[key].length ; jdx++) {
                    var fn = custom_data[key][jdx];
                    dst_dom.removeEventListener("click", fn);
                    dst_dom.addEventListener("click", fn);
                }
            } else {
                dst_dom[key] = custom_data[key];
            }
        }
    }
}


function get_cookie(keyword)
{ // TODO, might be replaced with js cookie library
    let out = null;
    if(document.cookie && document.cookie!=="") {
        const cookies = document.cookie.split(';');
        for(let idx = 0; idx < cookies.length; idx++) {
            const cookie = cookies[idx].trim();
            // does the cookie string begins with the given keyword
            if(cookie.substring(0, keyword.length + 1) === (keyword+"=")) {
                out = cookie.substring(keyword.length + 1);
                out = decodeURIComponent(out);
                break
            }
        }
    }
    return out;
}


function show_modal(elm, state) {
    if(state) {
        elm.classList.add("show");
        elm.style.display = "block";
        elm.style.background = "rgba(0,0,0,0.7)";
        elm.setAttribute('aria-hidden', false);
        elm.setAttribute('aria-modal', true);
    } else {
        elm.classList.remove("show");
        elm.style.display = "none";
        elm.style.background = "";
        elm.setAttribute('aria-hidden', true);
        elm.setAttribute('aria-modal', false);
    }
}


function event_handler_modal(evt)
{
    var target = evt.target;
    var callback = target._callback;
    if(callback) {
        var fn     = callback.fn;
        var kwargs = callback.kwargs;
        fn(kwargs);
    }
}


class APIconsumer {
    // the HTTP error response status that can be handled internally at this class
    err_http_resp_status = {429: this._handle_toomany_req};

    constructor(props) {
        if(!props.num_retry) {
            props.num_retry = 10;
        }
        this.props = props;
        this._init_csrf();
    }

    _parse_query_params(params) {
        var out = "";
        if(params != null) {
            out += "?";
            out += Object.keys(params)
                    .map(k => encodeURIComponent(k) +"="+ encodeURIComponent(params[k]))
                    .join('&');
        }
        return out;
    }

    _run_callbacks(data) { // invoke the specified list of callbacks
        var finish_cbs = this.props.finish_cbs;
        if(finish_cbs && (finish_cbs instanceof Array)) {
            for (var idx = 0; idx < finish_cbs.length; idx++) {
                finish_cbs[idx](data, this.props);
            }
        }
    }

    start(params) {
        // it takes time to load from remote API, better to use promise API like then()  as async approach.
        var url = this.props.api_base_url + this._parse_query_params(params);
        this.props._bak_query_params = params;
        fetch(url, this.props.req_opt)
            .then((res) => {
                    var err_handle_fn = this.err_http_resp_status[res.status];
                    this.props.http_resp_status = res.status;
                    if(err_handle_fn) {
                        err_handle_fn(this, res);
                        var errmsg = "handle this error internally, no need to parse response data";
                        throw new Error(errmsg);
                    }
                    return res.json();
                })
            .then((data) => this._run_callbacks(data))
            .catch((e) => {
                console.log("Error caught when fetching API: "+ e.message)
                var data = [{'non_form_error': e}]
                this._run_callbacks(data);
            });
    } // end of start
    
    _handle_toomany_req(consumer, resp) {
        var props = consumer.props;
        if(props.num_retry-- > 0) {
            setTimeout(
                function() {
                    consumer.start(props._bak_query_params);
                },
                80
            );
        } else {
            console.log("fail to consume API, still got too many request");
        }
    }

    _init_csrf() {
        if(this.props.req_opt.method == "GET") {
            return; // skip
        }
        // do not send CSRF token to other domains, TODO: consider cross-domain cases
        this.props.req_opt.mode = 'same-origin';
        // TODO, the code below only works for python Django, consider other web frameworks
        var name  = get_cookie("csrf_header_name");
        var tmp   = get_cookie("csrf_cookie_name");
        if(name && tmp) {
            var value = get_cookie(tmp);
            this.props.req_opt.headers[name] = value;
        } else {
            console.log('[warning] CSRF token not found in unauthenticated page');
        }
    }
} // end of class APIconsumer


class ChoiceFieldRender  extends React.Component {
    constructor(props) {
        super(props);
        this._unique_key = 1;
    }

    get_unique_key() { return this._unique_key++; }

    _load_options() {
        var out_objs = [];
        if(this.props.options) {
            var options = this.props.options;
            for (var idx = 0; idx < options.length; idx++) {
                var item = options[idx];
                var obj = React.createElement('option', {key:this.get_unique_key(),
                    value: item.value}, item.label);
                out_objs.push(obj);
            }
        }
        return out_objs;
    }

    render() {
        return this._load_options();
    }
} // end of class ChoiceFieldRender



function backup_bound_data(targets) {
    for (var idx = 0; idx < targets.length; idx++) {
        var target = targets[idx];
        if (target.value) {
            target.bak_value = target.value;
        }
        /// console.log("target.bak_value : "+ target.bak_value);
    }
}

function restore_bound_data(targets) {
    for (var idx = 0; idx < targets.length; idx++) {
        var target = targets[idx];
        if (target.bak_value) {
            target.value = target.bak_value;
        }
    }
}


function append_id_to_option_label(targets) {
    // TODO: this function exists only for debugging purpose, will be removed
    for (var idx = 0; idx < targets.length; idx++) {
        for (var jdx = 0; jdx < targets[idx].options.length; jdx++) {
            var curr_option = targets[idx].options[jdx];
            if(curr_option.value != '') {
                curr_option.text = '(' + curr_option.value + ') '+ curr_option.text
            }
        }
    }
} // end of append_id_to_option_label

function setup_single_select_elements(avail_data, props) {
    var targets = props.targets;
    append_id_to_option_label(targets);
    for (var idx = 0; idx < targets.length; idx++) {
        var new_opt = document.createElement('option');
        new_opt.text = '------';
        new_opt.value = '';
        new_opt.selected = true;
        targets[idx].add(new_opt);
    }
    restore_bound_data(targets);
}


function load_file_http(req) {
    // TODO: rename to load_static_file ? or deprecated then use APIconsumer instead ?
    // load  any type of file (csv/xml/json, REST API ?) through http server
    var client = new XMLHttpRequest();
    var async  = (req.async  != undefined) ? req.async : false;
    var uname  = (req.uname  != undefined) ? req.uname : false;
    var passwd = (req.passwd != undefined) ? req.passwd: false;
    client.open(req.method, req.file_path, async, uname, passwd);
    client.onreadystatechange = function() {
        if(this.status == 200) {
            if(this.readyState == 4) { // where requested file is loaded.
                // console.log("[load_loc_region] this.responseText :"+ this.responseText);
                var args = (req.cb_args == undefined || req.cb_args.succeed == undefined) ? null: req.cb_args.succeed;
                req.callback.succeed(this, args);
            }
        } else {
            var args = (req.cb_args == undefined || req.cb_args.failure == undefined) ? null: req.cb_args.failure;
            req.callback.failure(this, args);
        }
    }
    try {
        if(req.method == 'GET') {
            client.send();
        } else if (req.method == 'POST'){
            client.send(req.postdata);
        }
    } catch(err) { // TODO, report errors
        console.log("[load_file_http] check error type: "+ err);
    }
} // end of load_file_http


function convert_csv_to_json(raw_csv_in) {
    // split input raw string by ','  and clean up each item
    // str.replace(/"/gi, '').trim()
    var array_csv = raw_csv_in.split('\n').map(s => s.split(','));
    var json_out = [];
    array_csv[0] = array_csv[0].map(s => s.trim());
    for(var idx = 1; idx < (array_csv.length - 1); idx++) {
        if(array_csv[0].length != array_csv[idx].length) {
            throw "found row #"+ idx +" doesn't match the length of columns. ("+
                array_csv[0].length+" != "+ array_csv[idx].length +")";
        }
        var obj = {};
        array_csv[idx] = array_csv[idx].map(s => s.trim());
        for(var jdx = 0; jdx < array_csv[0].length; jdx++) {
            obj[array_csv[0][jdx]] = array_csv[idx][jdx];
        }
        json_out.push(obj);
    }
    return json_out;
}


// TODO: currently only work for django form module, will deprecated since many
// REST API libraries may not work this way
//// function get_mgt_formdata(tot_num_forms)
//// {   // note, no need to update INITIAL_FORMS
////     var init_cnt_val =  0; // TODO: given by backend
////     var out = {"INITIAL_FORMS": init_cnt_val, "TOTAL_FORMS": (tot_num_forms - init_cnt_val),
////                "MIN_NUM_FORMS": tot_num_forms , "MAX_NUM_FORMS":tot_num_forms, forms:[] };
////     return out;
//// }



class ReactBaseForm extends React.Component {

    static add_form(container, init_data) {
        var element = React.createElement(this, init_data); // "this" at here should be class type
        var wrapper = document.createElement('div');
        wrapper.id = "single-form-wrapper";
        ReactDOM.render(element, wrapper);
        container.appendChild(wrapper);
        return element;
    }
    // note that
    // (1) this contructor function will be called only once
    // (2) props has to be array of objects that store field data for each form.
    // (3) state.forms is used to record rendered forms so far in current page.
    constructor(props) {
        super(props);
        this._default_fields = null; // subclasses must contain a list of fields
        this.form_ref = React.createRef();
        this._unique_key = 1;
    }

    get_unique_key() { return this._unique_key++; }

    componentDidMount() {
        var dom_element = this.form_ref.current;
        if(dom_element == null) {
            throw "base form component error on mounting, DOM element not found";
        } else { // trace back to this react component, from DOM element
            dom_element.react_comp = this;
        }
        for(var key in this.btn_props) {
            var item = this.btn_props[key];
            if(item && item.ref) {
                var icon_dom = this.template_visual_comp.querySelector(item.icon_tag +"[id="+ item.icon_id +"]");
                var btn_dom  = item.ref.current;
                if(btn_dom && icon_dom) {
                    btn_dom.innerHTML = icon_dom.outerHTML;
                }
            }
        } // render button icons if they come from existing DOM elements
        for(var key in this.subform_data) {
            var d = this.subform_data[key];
            if(d.init && (d.init.length > 0)) {
                d.fn.addform(d.ref, d.init);
            }
        }
    } // end of componentDidMount

    // NOTE: all subclasses must override following function
    _new_single_form(init_form_data) { return null; }

    _get_all_fields_prop(default_fields, prop_in) {
        // TODO, verify
        var nonfield_err_item = {name:ReactBaseForm.non_field_errors_name, label:'non-field error',
            type:'text',  value:''};
        default_fields.push(nonfield_err_item);
        var prop_out = {};
        for(var idx = 0; idx < default_fields.length; idx++) {
            var df = default_fields[idx];
            prop_out[df.name] = {};
            if(df.tag   != undefined) { prop_out[df.name].tag   = df.tag;   }
            if(df.type  != undefined) { prop_out[df.name].type  = df.type;  }
            if(df.name  != undefined) { prop_out[df.name].name  = df.name;  }
            if(df.label != undefined) { prop_out[df.name].label = df.label; }
            if(df.value != undefined) { prop_out[df.name].defaultValue = df.value; }
            // init error list of all fields at here when this component is renderred at the first time
            df.ref = React.createRef();
            this._setup_errmsg_banner(df);
            prop_out[df.name].ref = df.ref;
            prop_out[df.name].key = this.get_unique_key();
        } // set up default attribute, on the fields with valid name
        for(var key in prop_in) {
            if(prop_out[key] == undefined) { continue; }
            for(var k2 in prop_in[key]) {
                prop_out[key][k2] = prop_in[key][k2];
            }
        }
        return prop_out;
    } // end of _get_all_fields_prop

    _setup_errmsg_banner(target) {
        target.errors = [];
        target.errmsg_banner = document.createElement("div");
        target.errmsg_banner.className = 'row p-1 card bg-danger';
        target.errmsg_banner.id        = 'field_errmsg_banner';
    }

    render() {
        this.template_visual_comp = null; 
        this.btn_props = {};
        this.subform_data = {};
        var _children   = this._new_single_form(this.props.form);
        var packed = React.createElement('div', {key: this.get_unique_key(), id: "packed", ref: this.form_ref,
                         className: this.props.className, children: _children},);
        return packed;
    }

    _generate_button(extra_props) {
        extra_props.key = this.get_unique_key();
        extra_props.ref = React.createRef();
        var link = React.createElement('button', extra_props);
        return link;
    }

    // special case of _generate_button() , each generated form could be closed
    _generate_close_button(label, extra_props) {
        var link = "";
        if(this.props.enable_close_btn == true) {
            extra_props.key =  this.get_unique_key();
            extra_props.ref =  React.createRef();
            extra_props.children = [label];
            extra_props.onClick = this._close_form.bind(this);
            link = React.createElement('button', extra_props);
        }
        return link;
    }

    _close_form() {
        var dom_element = this.form_ref.current; // id = packed
        var wrapper   = dom_element.parentNode;  // id = single-form-wrapper
        var container = wrapper.parentNode; // TODO: might cause problem if web component hierarchy changed
        var unmount_result = ReactDOM.unmountComponentAtNode(wrapper);
        container.removeChild(wrapper); // check wrapper.id  container.id
    }

    render_generic_subform_container(kwargs)
    {
        var subform_name = kwargs.subform_name;
        var _field = kwargs.field_in_prop;
        var out = kwargs.out;
        var fn  = kwargs.fn;

        var _children = [];
        if(_field.defaultValue && _field.defaultValue.length > 0) {
            _field.defaultValue.map((x) => {x.container = _field.ref; return x;});
        }
        this.subform_data[subform_name] = {ref:_field.ref, init:_field.defaultValue, fn:fn};
        this.btn_props['subform_'+ subform_name] = {className:"btn btn-outline-primary",
                title:'add one more '+ subform_name +' subform',icon_id:'add_form_icon', icon_tag:'svg',
                onClick: fn.addform.bind(this, _field.ref, null)  , children:["add form"], };
        var btn_addform = this._generate_button(this.btn_props['subform_'+ subform_name]);
        _children.push(
            React.createElement('div', {key: this.get_unique_key(), className:'row'},
                React.createElement('div', {key: this.get_unique_key(), className:'col-10'},
                    React.createElement('label', {key: this.get_unique_key()}, _field.label)
                ),
                React.createElement('div', {key: this.get_unique_key(), className:'col-2'}, btn_addform),
            )
        );
        _children.push(
           React.createElement('div', {key: this.get_unique_key(), className:"p-1", ref:_field.ref, id:subform_name},)
        );
        out.push( React.createElement('div', {key: this.get_unique_key(),  children: _children,
                        className:'form-group callout callout-info',}) );
    } // end of render_generic_subform_container


    static get_input_dom_value(dom_elm) {
        var out = {};
        var out_name  = dom_elm.name;
        var out_value = "";
        if(dom_elm.type == "checkbox") {
            out_value = dom_elm.checked ? 'true': 'false';
        } else {
            out_value = dom_elm.value;
        }
        out[out_name] = out_value;
        return  out;
    } // end of get_input_dom_value

    static get_select_dom_value(dom_elm) {
        var out = {};
        var out_name  = dom_elm.name;
        var out_value = "";
        out_value = Array.from(dom_elm.selectedOptions);
        out_value = out_value.map((x) => x.value);
        if(!dom_elm.multiple) {
            out_value = out_value[0];
        }
        out[out_name] = out_value;
        return  out;
    } // end of get_select_dom_value


    get_formdata() { // get form data as javascript object
        var out = {};
        var dfs = this._default_fields;
        for(var jdx = 0; jdx < dfs.length; jdx++) {
            //// console.log("[process on submit]  field "+ jdx +" , "+ dfs[jdx].name);
            var obj = dfs[jdx].ref.current;
            if(obj instanceof HTMLInputElement) {
                out = {...out, ...ReactBaseForm.get_input_dom_value(obj)};
            } else if(obj instanceof HTMLSelectElement) {
                out = {...out, ...ReactBaseForm.get_select_dom_value(obj)};
            } else  if(obj instanceof TagInstantSearchBox) {
                out = {...out, ...obj.get_formdata() };
            } else  if(obj instanceof ReactBaseForm) { // must contain one single subform (react coponent)
                out[dfs[jdx].name] = obj.get_formdata();
            } else  if(obj instanceof HTMLDivElement) { // must contain a list of subforms (react coponents)
                //// console.log(" .... subform list : object id "+ obj.id +",  "+ obj);
                var prefix = obj.id;
                out[prefix] = []; //// get_mgt_formdata(obj.children.length);
                for (var kdx = 0; kdx < obj.children.length; kdx++) {
                    var comp = obj.children[kdx].children[0].react_comp;
                    if(comp instanceof ReactBaseForm) {
                        //// out[prefix].forms.push(comp.get_formdata());
                        out[prefix].push(comp.get_formdata());
                    } else {
                        throw "a list must not contain different class types of instances"
                    }
                }
            }
        }
        return out;
    } // end of get_formdata


    _render_errmsg_banner(errsrc) {
        var form_dom = this.form_ref.current;
        //// var fd_dom   = errsrc.ref.current;
        var banner   = errsrc.errmsg_banner;
        if(errsrc.errors.length > 0 && errsrc.type != 'hidden') {
            var all_msgs = errsrc.label +": ";
            for(var idx = 0; idx < errsrc.errors.length; idx++) {
                all_msgs += errsrc.errors[idx] +",";
            }
            all_msgs += "<br>";
            banner.innerHTML = all_msgs;
            if(! form_dom.contains(banner)) {
                //// form_dom.appendChild(banner);
                form_dom.insertBefore(banner, form_dom.childNodes[0]);
            }
        }
    }

    _clean_errmsg_banners() {
        var banners = this.form_ref.current.querySelectorAll("div[id=field_errmsg_banner]");
        for(var idx = 0; idx < banners.length; idx++) {
            var parentnode = banners[idx].parentNode;
            if(parentnode) {
                parentnode.removeChild(banners[idx]);
            }
        }
    }

    report_error(err) {
        this._clean_errmsg_banners();   // clean up all previous error message banner
        return this._report_error(err); // update error message & re-render
    }

    _report_error(err) {
        var cnt = 0;
        if(err == undefined) {
            return cnt;
        }
        var non_field_errors = ReactBaseForm.non_field_errors_name; // TODO
        var dfs = this._default_fields;
        for(var idx = 0; idx < dfs.length; idx++) {
            var df = dfs[idx];
            df.errors.length = 0;
            if((err[df.name] == undefined) || (err[df.name].length == 0)) {
                continue;
            }
            var item = err[df.name][0];
            if(item instanceof Object) { // subform
                var obj = df.ref.current;
                if(obj instanceof ReactBaseForm) { // for one single subform
                    cnt += obj._report_error(err[df.name]);
                } else if (obj instanceof HTMLDivElement){ // for a list of subforms
                    for (var kdx = 0; kdx < obj.children.length; kdx++) {
                        var comp = obj.children[kdx].children[0].react_comp;
                        cnt += comp._report_error(err[df.name][kdx]);
                    }
                }
                if(err[df.name][non_field_errors]) { // handle non-field errors, TODO: may be deprecated ?
                    for(var jdx = 0; jdx < err[df.name][non_field_errors].length; jdx++) {
                        df.errors.push(err[df.name][non_field_errors][jdx]);
                    }
                    cnt++;
                    this._render_errmsg_banner(df);
                } // non-field error detected in current form
            } else {
                for(var jdx = 0; jdx < err[df.name].length; jdx++) {
                    df.errors.push(err[df.name][jdx]);
                }
                cnt++;
                this._render_errmsg_banner(df);
            }
        }
        return cnt;
    } // end of _report_error
} // end of class ReactBaseForm



class TagInstantSearchBox extends React.Component {
    constructor(props) {
        super(props);
        // even caller set props.ref when calling React.createElement(), the ref value will NOT
        // be sent here when instantiating React Component.
        this.search_value_ref = React.createRef();
        this.non_renderable_prop_keys = {'whitelist':0, 'dropdown':0, 'onmount':0, 'maxTags':0,
            'mode':0, 'evt_cb_add':0, 'evt_cb_remove':0, 'extract_prop_on_submit':0 };
    }
    
    componentDidMount() {
        var callback = this.props['onmount'];  // onmount property, if set, must be callable
        var dom_element = this.search_value_ref.current;
        if(dom_element == null) {
            throw "tag search object error on mounting, DOM element not found";
            return;
        }
        if(callback == undefined || callback == null) {
            callback = this.render_tag_search_box; // default callback
        }
        // link the third-party tag search object and this React component together,
        // for convinience to find one from the other.
        this.controller = callback(dom_element , this.props);
        this.controller.react_comp = this;
        dom_element.tagsearch = this.controller;
        dom_element.react_comp = this;
    }

    render() {
        var props = this.props;
        var _fields = {ref: this.search_value_ref};
        for(var key in props) {
            if(this.non_renderable_prop_keys.hasOwnProperty(key)) {
                continue;
            }
            _fields[key] = props[key];
        }
        _fields['hidden'] = true;
        // _fields['multiple'] = (this.props.mode == 'select') ? false : true;
        return  React.createElement('input', _fields ); // using select element will not process default value for Tagify
    }

    render_tag_search_box(dom_element, props) {
        // default function to render tag search component
        var init_keys = ['whitelist', 'dropdown', 'maxTags', 'mode'];
        var evt_key_map = {'add': 'evt_cb_add', 'remove': 'evt_cb_remove'};
        var _props = {}
        for(var idx = 0; idx < init_keys.length ; idx++) {
            var key = init_keys[idx];
            if(props[key] != undefined && props[key] != null) {
                _props[key] = props[key];
                if(key == 'whitelist') {
                    _props['enforceWhitelist'] = true;
                }
            }
        }
        var tagify = new Tagify(dom_element, _props);
        for(var key in evt_key_map) {
            if(props[evt_key_map[key]] != undefined && props[evt_key_map[key]] != null) {
                tagify.on(key, props[evt_key_map[key]]);
            }
        }
        return tagify;
    } // end of render_tag_search_box

    get_formdata() {
        var dom_elm = this.search_value_ref.current;
        var new_value = null;
        var out = {};
        out[dom_elm.name] = [];
        // loop through chosen tags, extract essential value for each tag to submit
        for(var idx = 0; idx < this.controller.value.length; idx++ ) {
            var item = this.controller.value[idx];
            if(item instanceof Object) {
                var extract_prop = this.props.extract_prop_on_submit;
                if(extract_prop == undefined) { extract_prop = 'value';}
                new_value = item[extract_prop];
            } else {
                new_value = item;
            }
            out[dom_elm.name].push(new_value);
        }
        if(this.props.mode == 'select') {
            out[dom_elm.name] = out[dom_elm.name][0]? out[dom_elm.name][0]: '';
        }
        return out;
    }


    update_nxt_lvl_comp(whitelist)
    {   // update next-level tag search object if exists 
        var nxt_lvl_comp = TagInstantSearchBox.find_next_lvl_comp(this);
        nxt_lvl_comp.controller.settings.whitelist = whitelist;
        nxt_lvl_comp.controller.removeAllTags();
    }

    static find_next_lvl_comp(curr_lvl_comp) {
        var out = null;
        if((curr_lvl_comp != null) && (curr_lvl_comp instanceof this)) {
            var nxt_lvl_elm = curr_lvl_comp.props.nxt_lvl_elm;
            //// console.log("country  comp props keys : "+ Object.keys(curr_lvl_comp.props));
            if((nxt_lvl_elm.ref.current != null) && (nxt_lvl_elm.ref.current instanceof this)) {
                out = nxt_lvl_elm.ref.current;
            }
        }
        return out;
    }
} // end of class TagInstantSearchBox


function get_default_form_props(dom_ref, addform_fn) {
    var out = {}
    out.container = dom_ref; //// out.dom_ref = dom_ref;
    out.className = "p-1";
    out.btns = {addform: {enable:true, evt_cb: addform_fn}, closeform:{},};
    out.enable_close_btn = true;
    out.form = {};
    return out;
}


// TODO, should use keyword arguments instead
function api_data_ready_cb(avail_data, props) {
    var forms =  props.caller; // should be html element that represents form set
    if((!forms) || (!forms.chk_load_api_done_cbs)) {
        return;
    }
    //// console.log("API data load, start checking if all data is ready. "+ props);
    for(var key in forms.chk_load_api_done_cbs) {
        var chk_fn = forms.chk_load_api_done_cbs[key];
        if(!chk_fn()) {
            return;
        }
    }
    var df_props = [];
    // caller must offer callback function to load edit data (for bulk update case)
    if(forms.load_edit_data_cb) {
        df_props = forms.load_edit_data_cb(forms);
    } else {
        df_props[0] = forms.get_default_form_props(forms) ;
        df_props[0].enable_close_btn = false;
    }
    forms.append_new_form(forms, df_props);
    console.log("all necessary API data loaded");
}


function render_form_window_btns(formobj)
{ // only create 2 buttons for form window manipulation : add-form, close-form
    var tool_btns = [];
    var btns = formobj.props.btns;
    formobj.template_visual_comp = document.getElementById('template_visual_comp');
    if(btns && btns.addform &&  btns.addform.enable) {
        formobj.btn_props.add_form = {className:btns.addform.className, children:["add form"], title:'add one more form',
             onClick: btns.addform.evt_cb.bind(formobj, formobj.props.container, null),
             icon_id:'add_form_icon', icon_tag:'svg', } ;
        tool_btns.push(formobj._generate_button(formobj.btn_props.add_form));
    }
    formobj.btn_props.close_form = {className:btns.closeform.className, title:"close the form", icon_id:'close_form_icon', icon_tag:'svg', };
    tool_btns.push(formobj._generate_close_button("close", formobj.btn_props.close_form));
    return tool_btns;
}


function check_submit_result_cb(avail_data, props) {
    var forms = props.forms;
    var errors = avail_data;
    var nonfield_errmsg_banner = forms.parentNode.querySelector("div[id=nonfield_errmsg_banner]");
    nonfield_errmsg_banner.hidden = true;
    if(props.http_resp_status == 201 || props.http_resp_status == 200) {
        if(forms.dataset.is_single_form) {
            var single_form = forms.children[0].children[0];
            single_form.react_comp._clean_errmsg_banners();
        } else {
            for(var idx = 0; idx < errors.length; idx++) {
                var single_form = forms.children[idx].children[0];
                single_form.react_comp._clean_errmsg_banners();
            }
        }
        console.log('succeeded to save data, ready to redirect to '+ props.success_url_redirect +' ...'); 
        if(props.success_url_redirect) {
            window.location.href = props.success_url_redirect;
        }
    } else {
        var total_error_cnt = 0;
        if(forms.dataset.is_single_form) {
            var single_form = forms.children[0].children[0];
            var has_err = Object.keys(errors).length;
            if(has_err == 0) {
                single_form.react_comp._clean_errmsg_banners();
            } else {
                single_form.react_comp.report_error(errors);
            }
        } else {
            for(var idx = 0; idx < errors.length; idx++) {
                var single_form = forms.children[idx].children[0];
                var has_err = Object.keys(errors[idx]).length;
                if(has_err == 0) {
                    single_form.react_comp._clean_errmsg_banners();
                } else {
                    total_error_cnt += single_form.react_comp.report_error(errors[idx]);
                }
            }
            if(errors.length == undefined) {
                var key = ReactBaseForm.non_field_errors_name;
                var non_field_errors = errors[key];
                var all_msgs = key +" : " + non_field_errors.join('<br>');
                nonfield_errmsg_banner.innerHTML = all_msgs;
                nonfield_errmsg_banner.hidden = false;
            }
        }
    } // end of response status 4xx 5xx
} // end of check_submit_result_cb


function on_submit_forms(evt)
{
    var forms = evt.target.forms;
    var dataset = evt.target.dataset;
    var query_params = evt.target.query_params;
    var body_data = [];
    var finish_callbacks = [check_submit_result_cb,];
    var tmp = null;
    if(forms.dataset.is_single_form && forms.children.length != 1) {
        throw "`is_single_form` is set while submitting multiple forms";
    }
    for(var idx = 0; idx < forms.children.length; idx++) {
        var single_form = forms.children[idx].children[0];
        if((single_form.react_comp != undefined) && (single_form.react_comp != null)) {
            tmp = single_form.react_comp.get_formdata();
            body_data.push(tmp);
        } else {
            throw "failed to find react component from DOM element";
        }
    }
    if(forms.dataset.is_single_form) {
        body_data = body_data[0];
    }
    body_data = JSON.stringify(body_data);
    if(evt.target.finish_callbacks) {
        finish_callbacks = finish_callbacks.concat(evt.target.finish_callbacks)
    } // append extra callbacks specified by application caller
    var headers = {'content-type':'application/json', 'accept':'application/json'}; 
    var req_opt = {method:dataset.api_mthd, headers: headers,  body: body_data,};
    var submit_api = new APIconsumer({api_base_url: dataset.api_base_url, req_opt:req_opt, forms:forms, caller:evt.target,
                         success_url_redirect: dataset.success_url_redirect, finish_cbs: finish_callbacks });
    submit_api.start(query_params);
} // end of on_submit_forms


export { 
    cvt_list_to_dict, APIconsumer, ChoiceFieldRender, backup_bound_data, restore_bound_data,
    append_id_to_option_label, setup_single_select_elements, load_file_http, convert_csv_to_json,
    ReactBaseForm, TagInstantSearchBox, truncate_text, update_doms_props, render_form_window_btns,
    get_default_form_props, on_submit_forms, api_data_ready_cb, show_modal, event_handler_modal,
    get_cookie,
};

