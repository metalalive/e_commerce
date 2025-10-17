import json
import os
import pwd
import grp
import shutil
from typing import Generator as TyGenerator
from pathlib import Path as PyPath
from jinja2 import Environment, FileSystemLoader


class NginxConfigGenerator:
    def start(self, argv: list):
        assert len(argv) >= 4, "arguments error"
        cdn_cfg_path, uname, usrgrp, num_backends = argv[:4]
        num_backends = int(num_backends)
        assert num_backends > 0, "there must be at least one backend added to Nginx config"
        backends_cfg_path = argv[4 : 4 + num_backends]
        pwd.getpwnam(uname)
        grp.getgrnam(usrgrp)
        cdn_cfg_path = PyPath(cdn_cfg_path)
        assert cdn_cfg_path.exists(), "config file not exists"
        cdn_cfg_root = None
        with open(cdn_cfg_path, "r") as f:
            cdn_cfg_root = json.load(f)
        curr_work_path = PyPath().resolve()
        cfg_dst_path = curr_work_path.joinpath(cdn_cfg_root["proxy"]["genfile_path"])
        env = Environment(loader=FileSystemLoader(cdn_cfg_root["proxy"]["template_path"]))
        upstream_param_gen = self._generate_upstream_server(backends_cfg_path)
        renderred = self._render_entry(
            template=env.get_template("entry.conf"),
            uname=uname,
            usrgrp=usrgrp,
            cdn_cfg_root=cdn_cfg_root,
            ups_param_gen=upstream_param_gen,
            curr_work_path=curr_work_path,
        )
        self.write_renderred_to_file(dst_path=cfg_dst_path, wdata=renderred, filename="entry.conf")
        renderred = self.render_location_common(
            bkd_cfg_path=backends_cfg_path[0],
            template=env.get_template("loc_proxy_common.conf"),
            curr_work_path=curr_work_path,
        )
        self.write_renderred_to_file(
            dst_path=cfg_dst_path, wdata=renderred, filename="loc_proxy_common.conf"
        )
        print("rendering Nginx config file ...... done")

    def _render_entry(
        self,
        template,
        cdn_cfg_root: dict,
        curr_work_path: PyPath,
        uname: str,
        usrgrp: str,
        ups_param_gen: TyGenerator,
    ):
        pxycfg = cdn_cfg_root["proxy"]
        pxylmt = pxycfg["limit"]
        pxycch = pxycfg["cache"]
        pxyssl = pxycfg["ssl"]
        qps = pxylmt["reqs_per_sec"]
        cache_basepath = pxycch["basepath"]
        cache_inactive_minutes = pxycch["inactive_mins"]
        cache_sz_mb = pxycch["max_space_mbytes"]
        cacheentry_lock_timeout = pxycch["lock_timeout_secs"]
        pxysrv_cert_fullpath = curr_work_path.joinpath(pxyssl["cert_file"])
        pxysrv_pkey_fullpath = curr_work_path.joinpath(pxyssl["privkey_file"])
        pxysrv_ssl_sess_timeout_mins = int(pxyssl["session_timeout_secs"] / 60)
        kwargs = {
            "os_username": uname,
            "os_user_group": usrgrp,
            "logging_level": pxycfg["logging_level"],
            "max_num_conns": pxycfg["max_num_conns"],
            "reqs_per_sec": qps,
            "qps_minus1": qps - 1,
            "pxy_cch_basepath": cache_basepath,
            "pxy_cch_inactive_mins": cache_inactive_minutes,
            "pxy_cch_max_space_mb": cache_sz_mb,
            "pxy_cch_lock_timeout": cacheentry_lock_timeout,
            "pxy_srv_port": pxycfg["port"],
            "pxy_srv_hostname": pxycfg["host"],
            "pxy_srv_ssl_cert_path": str(pxysrv_cert_fullpath),
            "pxy_srv_ssl_privkey_path": str(pxysrv_pkey_fullpath),
            "pxy_srv_ssl_sess_timeout_mins": pxysrv_ssl_sess_timeout_mins,
            "cps_nstream": pxylmt["conns_per_sec"]["non-stream"],
            "cps_stream": pxylmt["conns_per_sec"]["stream"],
            "http_keepalive_timeout_secs": pxycfg["http_keepalive_timeout_secs"],
        }
        return template.render(backend_srv_iteration=ups_param_gen, **kwargs)

    def _generate_upstream_server(self, bkd_cfg_paths: list):
        ups_node_cls = type(
            "upstream_node_class",
            (object,),
            {
                "hostname": None,
                "port": 0,
                "max_conns": 0,
                "max_fails": 0,
                "retry_after_unavail_secs": 0,
            },
        )
        for p in bkd_cfg_paths:
            bkd_cfg_root = None
            with open(p, "r") as f:
                bkd_cfg_root = json.load(f)
            if bkd_cfg_root is None:
                continue
            _max_num_conns = bkd_cfg_root["max_connections"]
            for l in bkd_cfg_root["listen"]:
                obj = ups_node_cls()
                obj.hostname = l["host"]
                setattr(obj, "port", l["port"])
                setattr(obj, "max_fails", l["max_fails"])
                setattr(obj, "retry_after_unavail_secs", l["retry_after_unavail_secs"])
                setattr(obj, "max_conns", _max_num_conns)
                yield obj

    def render_location_common(self, template, bkd_cfg_path: str, curr_work_path: PyPath):
        bkd_cfg_root = None
        with open(bkd_cfg_path, "r") as f:
            bkd_cfg_root = json.load(f)
        if bkd_cfg_root is not None:
            ca_cfg = bkd_cfg_root["ca"]
            origsrv_ca_fullpath = curr_work_path.joinpath(ca_cfg["cert_file"])
            origsrv_pkey_fullpath = curr_work_path.joinpath(ca_cfg["privkey_file"])
            kwargs = {
                "pxy_upstream_ssl_ca_path": origsrv_ca_fullpath,
                "pxy_upstream_ssl_privkey_path": origsrv_pkey_fullpath,
            }
            return template.render(**kwargs)

    def write_renderred_to_file(self, dst_path: PyPath, filename: str, wdata: str):
        if dst_path.exists() is False:
            os.makedirs(dst_path, mode=0o755, exist_ok=True)
        file_fullpath = dst_path.joinpath(filename)
        with open(file_fullpath, "w") as f:
            f.write(wdata)


## end of class NginxConfigGenerator


class NginxPathSetup:
    def start(self, argv: list):
        assert len(argv) == 2, "arguments error"
        cdn_cfg_path, ngx_install_path = argv
        cdn_cfg_path = PyPath(cdn_cfg_path)
        ngx_install_path = PyPath(ngx_install_path)
        assert cdn_cfg_path.exists(), "cdn_cfg_path not exists"
        assert ngx_install_path.exists(), "ngx_install_path not exists"
        curr_work_path = PyPath().resolve()
        cdn_cfg_root = None
        with open(cdn_cfg_path, "r") as f:
            cdn_cfg_root = json.load(f)
        self._copypath(
            curr_work_path,
            ngx_install_path=ngx_install_path,
            ngx_genfile_path=cdn_cfg_root["proxy"]["genfile_path"],
        )
        self._init_cache_path(ngx_install_path, cdn_cfg_root["proxy"]["cache"]["basepath"])
        print("setup paths for Nginx server ...... done")

    def _copypath(self, curr_work_path: PyPath, ngx_install_path: PyPath, ngx_genfile_path: str):
        ngx_genfile_path = curr_work_path.joinpath(ngx_genfile_path)
        assert (
            ngx_genfile_path.exists() and ngx_genfile_path.is_dir()
        ), "ngx_genfile_path not exists"
        folder_name = ngx_genfile_path.name
        ngx_cfg_dstpath = ngx_install_path.joinpath("conf").joinpath(folder_name)
        if ngx_cfg_dstpath.is_dir() and ngx_cfg_dstpath.exists():
            shutil.rmtree(ngx_cfg_dstpath)
        shutil.copytree(ngx_genfile_path, ngx_cfg_dstpath)

    def _init_cache_path(self, ngx_install_path: PyPath, cch_base: str):
        fullpath = ngx_install_path.joinpath(cch_base)
        os.makedirs(fullpath, mode=0o750, exist_ok=True)


## end of class NginxPathSetup


__all__ = ["NginxConfigGenerator", "NginxPathSetup"]
