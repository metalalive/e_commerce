
export function clear_array(arr) {
    arr.length = 0;
}
export function clear_props_object(obj) {
    for(const prop of Object.getOwnPropertyNames(obj)) {
        delete obj[prop];
    }
}

export function get_input_dom_value(dom_elm) {
    // dom_elm.name;
    var out_value = "";
    if(dom_elm.type === "checkbox") {
        out_value = dom_elm.checked ? 'true': 'false';
    } else {
        out_value = dom_elm.value;
    }
    return   out_value;
} // end of get_input_dom_value


export function get_select_dom_value(dom_elm) {
    var out_value = "";
    out_value = Array.from(dom_elm.selectedOptions);
    out_value = out_value.map((x) => x.value);
    if(!dom_elm.multiple) {
        out_value = out_value[0];
    }
    return  out_value;
} // end of get_select_dom_value


export function set_input_dom_value(dom_elm, value) {
    if(dom_elm.type === "checkbox") {
        dom_elm.checked = value ? 'true': 'false';
    } else {
        dom_elm.value = value ? value : '';
    }
}

export function set_select_dom_value(dom_elm, value) {
    throw Error("not implemented yet");
}

export function get_cookie(keyword)
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


export class APIconsumer {
    constructor(props) {
        if(!props.num_retry) {
            props.num_retry = 10;
        }
        if(!props.wait_interval_ms){
            props.wait_interval_ms = 240;
        }
        // the HTTP error response status that can be handled internally at this class
        this.err_http_resp_status = {
            429: this._handle_toomany_req.bind(this),
            //404: this._handle_toomany_req.bind(this),
        };
        this.props = props;
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

    async start(req_args) {
        if(isNaN(req_args.num_retry)) {
            req_args.num_retry = this.props.num_retry;
        }
        this._init_csrf(req_args.req_opt);
        var promise = this._start(req_args);
        if(true) {
            await promise;
        }
        return promise;
    }
    
    _start(req_args) {
        // it takes time to load from remote API, better to use promise API like then()  as async approach.
        var url = req_args.base_url + this._parse_query_params(req_args.params);
        var promise = fetch(url, req_args.req_opt)
            .then(async (res) => {
                var inner_promise = null;
                res.req_args = req_args;
                if(res.status >= 400) {
                    var err_handle_fn = this.err_http_resp_status[res.status];
                    if(err_handle_fn) {
                        inner_promise = err_handle_fn(res);
                    } else {
                        inner_promise = res.json().then((data) => {
                            this._run_callbacks(req_args.callbacks.unhandled_error_response, data, res);
                        }).catch((e) => {
                            this._run_callbacks(req_args.callbacks.unhandled_error_response, null, res);
                        });
                    }
                } else {
                    inner_promise = res.json().then((data) => {
                        this._run_callbacks(req_args.callbacks.succeed, data, res);
                    }).catch((e) => {
                        this._run_callbacks(req_args.callbacks.succeed, null, res);
                    });
                }
                if(inner_promise) {
                    await inner_promise;
                }
            }).catch((e) => {
                var data = [{'non_form_error': e}]
                var res = {status:0 , req_args: req_args, exception:e};
                this._run_callbacks(req_args.callbacks.unhandled_exception, data, res);
            });
        return promise;
    } // end of start
    
    _handle_toomany_req(res) {
        var promise = null;
        if(!isNaN(res.req_args.num_retry) && res.req_args.num_retry-- > 0) {
            var bound_fn_restart = this._start.bind(this);
            promise = new Promise((resolve) => {setTimeout(resolve, this.props.wait_interval_ms);});
            promise.then(() => {  bound_fn_restart(res.req_args); });
        } else {
            console.log("fail to consume API, still got too many request");
            this._run_callbacks(res.req_args.callbacks.server_busy, null, res);
        }
        return promise;
    }
    
    _run_callbacks(callbacks ,data, res) { // invoke the specified list of callbacks
        if(callbacks && (callbacks instanceof Array)) {
            for (var idx = 0; idx < callbacks.length; idx++) {
                callbacks[idx](data, res, this.props);
            }
        }
    }

    _init_csrf(req_opt) {
        if(req_opt.method == "GET") {
            return; // skip
        }
        // do not send CSRF token to other domains, TODO: consider cross-domain cases
        if(req_opt.mode == undefined) {
            req_opt.mode = 'same-origin';
        }
        // TODO, the code below only works for python Django, consider other web frameworks
        var name  = get_cookie("csrf_header_name");
        var tmp   = get_cookie("csrf_cookie_name");
        if(name && tmp) {
            var value = get_cookie(tmp);
            req_opt.headers[name] = value;
        } else {
            console.log('[warning] CSRF token not found in unauthenticated page');
        }
    }

    static copy_callbacks(origin, cp_types) {
        // partial shallow copy on the callbacks
        const DEFAULT_CALLBACK_TYPES = ['unhandled_error_response', 'succeed', 'unhandled_exception', 'server_busy'];
        cp_types = cp_types.filter((t) => (DEFAULT_CALLBACK_TYPES.indexOf(t) >= 0));
        let _origin = origin;
        if(_origin === undefined || _origin === null) {
            _origin = {};
        }
        var copied = {... _origin}; // shallow copy all properties first
        for(var idx = 0; idx <= cp_types.length; idx++) {
            var prop = cp_types[idx];
            var cp_item = _origin[prop] ? [... _origin[prop]]: []; // shallow copy to the array items
            copied[prop] = cp_item;
        }
        return copied;
    }

    static serialize(objlist, valid_field_names, reducer) {
        var out = null;
        if(objlist.length === undefined) {
            objlist = Object.entries(objlist).map(kv => kv[1]);
        }
        out = objlist.map((item) => {
            var picked = {};
            for(var key in item) {
                if(valid_field_names.indexOf(key) >= 0) {
                    picked[key] = item[key];
                } // TODO, would argument `valid_field_names` be unecessary ?
            }
            return picked;
        });
        return out.length > 0 ? JSON.stringify(out, reducer) : null;
    }
} // end of class APIconsumer


export function patch_string_prototype() {
    if (String.prototype.format) {
        return;
    }
    String.prototype.format = function() {
      var args = arguments;
      return this.replace(
          /{(\d+)}/g,
          function(match, number) { 
              return typeof args[number] != 'undefined' ? args[number]: match;
          });
    };
}


export function _instant_search(evt) {
    let invoker = evt.nativeEvent.target;
    var keyword = null;
    var api_url = null;
    if(invoker instanceof HTMLInputElement) {
        if (evt.code == "Enter") {
            console.log('start instant search ... keycode:'+ evt.code);
            keyword = invoker.value;
            api_url = invoker.dataset.api_url;
        }
    } else if(invoker instanceof HTMLButtonElement) {
        var txtfield = invoker.parentNode.querySelector("input");
        keyword = txtfield.value;
        api_url = txtfield.dataset.api_url;
    }
    if (keyword && api_url) {
        // this event handling function must be bound with any object
        // that implements callable attribute `search_api_fn()`
        //let tree_ref = this.current;
        //tree_ref.search(keyword, api_url);
        this.search_api_fn(keyword, api_url)
    }
}

export function toggle_visual_elm_showup(dom, menu_classname, dom_showup_classname) {
    let target = dom.querySelector("."+menu_classname);
    let class_list = Array.from(target.classList);
    if(class_list.indexOf(dom_showup_classname) >= 0) {
        target.classList.remove(dom_showup_classname);
    } else {
        target.classList.add(dom_showup_classname);
    }
}



