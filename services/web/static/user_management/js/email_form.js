import * as toolkit from "/static/js/toolkit.js";


export class EmailForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',   type:'hidden', value:''}, // id specific to the email address
            {name:'uid',  type:'hidden', value:''}, // id bound with specific user, used only in update scene
            {name:'addr', type:'text', value:'', label:'mail address'},
        ];
    }

    _new_single_form(init_form_data) {
        var _children = [];
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        _children.push(
            React.createElement('input', _fields['id']),
            React.createElement('input', _fields['uid']),
            React.createElement('div', {key: this.get_unique_key(), className:'col-9'},
                React.createElement('input', _fields['addr'])
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-3',
                children:toolkit.render_form_window_btns(this)},),
        );
        var row = React.createElement("div", {key: this.get_unique_key(), className:'row p-0',children:_children},)
        return [row];
    }
} // end of class EmailForm 


export function get_default_form_props(dom_ref) {
    var out = toolkit.get_default_form_props(dom_ref, append_new_form);
    out.btns.addform.className   = "btn btn-sm btn-outline-primary p-1";
    out.btns.closeform.className = "btn btn-sm btn-outline-primary p-1";
    out.form['uid']  = {};
    out.form['id']   = {};
    out.form['addr'] = {defaultValue: '', className:"form-control", placeholder:'e.g. mailuser@domainname.com',};
    return out;
}

export function append_new_form(dom_ref, props) {
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
        EmailForm.add_form(container, props[idx]);
    }
}


export function render_with_data(data) {
    var out = [];
    for (var jdx = 0; jdx < data.length; jdx++) {
        var prop = get_default_form_props(null);
        prop.form['uid'].defaultValue  = data[jdx].uid;
        prop.form['id'].defaultValue   = data[jdx].id;
        prop.form['addr'].defaultValue = data[jdx].addr;
        out.push(prop);
    } // end of inner loop
    return out;
}

// ---------------------------------------------------------

export function launch_mail_selector(kwargs)
{
    var props = kwargs.api;
    var ids = kwargs.users.map(x => x.id);
    props.query_params.ids = ids.join(",");
    if(!props.callbacks) {
        props.callbacks = [];
    }
    props.callbacks.push(_render_mail_selector);
    var load_api_fn = props.load_api_fn;
    delete props.load_api_fn;
    load_api_fn(props); // consume API asynchronously

    var caller = props.caller;
    var custom_modal = document.getElementById(caller.dataset.modal);
    caller.modal = custom_modal;
    custom_modal._users = kwargs.users; // used in _render_mail_selector
    // setup buttons in the modal
    _launch_modal(custom_modal, kwargs.modal);
}


function _enable_btn_fn(btn, enable)
{
    if(enable) {
        btn.addEventListener('click', toolkit.event_handler_modal);
        btn.addEventListener('click', _clean_rendered_data);
        btn.classList.remove("disabled");
    } else {
        btn.removeEventListener('click', toolkit.event_handler_modal);
        btn.removeEventListener('click', _clean_rendered_data);
        btn.classList.add("disabled");
    }
}


function _launch_modal(modal, kwargs)
{
    var btns = {
        dismiss: modal.querySelector("button[id="+ modal.dataset.btn_dismiss +"]"),
        close:   modal.querySelector("button[id="+ modal.dataset.btn_close +"]"),
        apply:   modal.querySelector("button[id="+ modal.dataset.btn_apply +"]"),
    };
    modal.btns = btns;
    var title = modal.querySelector("[class=modal-title]");
    if(title && kwargs.title) {
        title.innerText = kwargs.title;
    }
    var errmsg_banner = modal.querySelector("div[id=error_message]");
    if(errmsg_banner && kwargs.error_message){
        errmsg_banner.innerHTML = kwargs.error_message;
    }
    for(var key in btns) {
        if(!btns[key]) {
            continue;
        }
        btns[key]._modal_within = modal;
        btns[key]._callback = kwargs.btns[key].callback;
        if(kwargs.btns[key].label) {
            btns[key].innerText = kwargs.btns[key].label;
        }
        _enable_btn_fn(btns[key], (key != 'apply'));
    } // end of for loop
    toolkit.show_modal(modal, true);
} // end of _launch_modal


function enable_apply_btn(evt) {
    var tagsearch = evt.detail.tagify;
    var chosen_tags = tagsearch.value;
    var comp = tagsearch.react_comp;
    if(chosen_tags.length > 0) {
        var btn_apply = comp.props.modal.btns.apply;
        btn_apply.addEventListener('click', _collect_chosen_mails);
        _enable_btn_fn(btn_apply , true);
    }
    // TODO, use get_formdata() to get selected items, pas it to user callbacks
}

function disable_apply_btn(evt) {
    var tagsearch = evt.detail.tagify;
    var chosen_tags = tagsearch.value;
    var comp = tagsearch.react_comp;
    if(chosen_tags.length == 1) {
        var btn_apply = comp.props.modal.btns.apply;
        btn_apply.removeEventListener('click', _collect_chosen_mails);
        _enable_btn_fn(btn_apply , false);
    }
}


function _render_mail_selector(avail_data, props) {
    const dropdown = {maxItems:10, classname:'tags-look', enabled:0, closeOnSelect:false};
    if (!avail_data && !avail_data.result) {
        return;
    }
    if(avail_data.result) {
        avail_data = avail_data.result;
    }
    var num_items_done = 0;
    var caller = props.caller;
    // render loaded API data to data container in the modal
    var users = caller.modal._users;
    var data_container = caller.modal.querySelector("div[id=data_container]");
    for(var idx = 0; idx < avail_data.length; idx++)
    {
        if(avail_data[idx].emails.length == 0) {
            continue;
        }
        var selected_user = users.find(x => x.id == avail_data[idx].id);
        avail_data[idx].emails.map((x) => {x.value = x.addr; delete x.addr; return x;});
        var tagsearch_props = {defaultValue: [], label:'from existing group', whitelist: avail_data[idx].emails,
                 extract_prop_on_submit:'uid', className:'tagify--custom-dropdown', placeholder:'choose user email',
                 evt_cb_add: enable_apply_btn, evt_cb_remove: disable_apply_btn, name:'email', mode: 'select',
                 dropdown: dropdown, key:idx, modal: caller.modal, user_id: selected_user.id };
        var tagsearch_elm = React.createElement(toolkit.TagInstantSearchBox, tagsearch_props);
        var item_wrapper = document.createElement('div');
        var tag_wrapper  = document.createElement('div');
        var namelabel    = document.createElement('label');
        namelabel.innerHTML = selected_user.first_name +" "+ selected_user.last_name;
        ReactDOM.render(tagsearch_elm, tag_wrapper);
        item_wrapper.appendChild(namelabel);
        item_wrapper.appendChild(tag_wrapper);
        data_container.appendChild(item_wrapper);
        num_items_done++;
    } // end of loop
    var errmsg_banner = caller.modal.querySelector("div[id=error_message]");
    errmsg_banner.hidden = (num_items_done > 0);
} // end of _render_mail_selector


function _collect_chosen_mails(evt) {
    // of all event listener functions in the apply button, this function must be called first
    var target = evt.target;
    var modal  = target._modal_within;
    var data_container = modal.querySelector("div[id=data_container]");
    var _chosen_emails = [];

    for(var idx = 0; idx < data_container.children.length; idx++) {
        var item_wrapper = data_container.children[idx];
        var tag_wrapper  = item_wrapper.querySelector('div');
        var tagsearch_comp = tag_wrapper.querySelector('input').react_comp;
        var data = tagsearch_comp.get_formdata();
        data.user_id = tagsearch_comp.props.user_id;
        _chosen_emails.push(data);
    }
    target._callback.kwargs._chosen_emails = _chosen_emails;
}


function _clean_rendered_data(evt)
{
    var target = evt.target;
    var modal  = target._modal_within;
    var data_container = modal.querySelector("div[id=data_container]");
    while(data_container.children.length > 0)
    {
        var item_wrapper = data_container.lastChild;
        var tag_wrapper  = item_wrapper.querySelector('div');
        data_container.removeChild(item_wrapper);
        item_wrapper.removeChild(tag_wrapper);
        var unmount_result = ReactDOM.unmountComponentAtNode(tag_wrapper);
    }
    toolkit.show_modal(modal, false);
} // end of _clean_rendered_data


// of all event listener functions in the apply button, this function must be called at first
