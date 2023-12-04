import * as toolkit from "/static/js/toolkit.js";
// TODO, deprecated, will use table-grid library instead to render backend data


function evt_submit_search(evt) {
    var submit_flg = false;
    if(evt.type == "keyup" && evt.keyCode == 13) {
        submit_flg = true;
    } else if (evt.type == "click") {
        submit_flg = true;
    }
    if(!submit_flg) { return }
    var props = evt.target.props;
    var tagname = evt.target.tagName.toLocaleLowerCase();
    var inputtype = evt.target.type;
    var search_field = null;
    if(props == undefined) { return; }
    if(tagname == "button") {
        search_field = evt.target.search_field;
    } else if (tagname == "input" && inputtype == "search") {
        search_field = evt.target;
    }
    if(!search_field) { return; }
    props.query_params.page = 1;
    props.query_params.page_size = props.default_page_size;
    props.query_params.search = search_field.value;
    props.load_api_fn(props);
}

// TODO filter panel , e.g. by time period, multiple keywords with specific field name
// check third party search engine to use
function init_search_bar(new_dom, api, IDs) {
    var btn_data = {};
    var search_btn = new_dom.querySelector("button[id="+ IDs.button +"]");
    var search_field = new_dom.querySelector("input[id="+ IDs.textfield +"]");
    if(api) {
        btn_data[IDs.button]    = {props: api, search_field:search_field, onclick: evt_submit_search, };
        btn_data[IDs.textfield] = {props: api, onkeyup: evt_submit_search};
    } else {
        btn_data[IDs.button]    = {hidden: true,};
        btn_data[IDs.textfield] = {hidden: true,};
    }
    toolkit.update_doms_props([search_field, search_btn], btn_data);
}


export {init_search_bar, };

