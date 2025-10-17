#include <assert.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <uv.h>

#include "app_cfg.h"
#include "utils.h"
#include "transcoder/video/mp4.h"

#define RUNNER_CREATE_FOLDER(fullpath)          mkdir(fullpath, S_IRWXU)
#define RUNNER_OPEN_WRONLY_CREATE_USR(fullpath) open(fullpath, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR)

#define LOCAL_TMPBUF_BASEPATH       "tmp/buffer/media/test"
#define UNITTEST_FOLDER_NAME        "utest"
#define PRELOAD_FOLDER_NAME         "mock_preload"
#define PRELOAD_SRCFILE_FOLDER_NAME "src"
#define LOCAL_TMPBUF_NAME           "local_tmpbuf"

#define UNITTEST_FULLPATH    LOCAL_TMPBUF_BASEPATH "/" UNITTEST_FOLDER_NAME
#define PRELOAD_BASEPATH     UNITTEST_FULLPATH "/" PRELOAD_FOLDER_NAME
#define PRELOAD_SRC_BASEPATH PRELOAD_BASEPATH "/" PRELOAD_SRCFILE_FOLDER_NAME
#define LOCAL_TMPBUF_PATH    PRELOAD_BASEPATH "/" LOCAL_TMPBUF_NAME

#define MP4__ATOM_TYPE_FTYP "ftyp"
#define MP4__ATOM_TYPE_FREE "free"
#define MP4__ATOM_TYPE_MOOV "moov"
#define MP4__ATOM_TYPE_MDAT "mdat"

#define MP4__ATOM_BODY_FTYP "interchangeable_receiver"
#define MP4__ATOM_BODY_MOOV "assembly_line_worker_progress_millions_of_mountains"
#define MP4__ATOM_BODY_MDAT "loose_coupling_between_app_server_and_SQLdatabase_&_nosql_storage"

#define MP4_ATOM_FTYP MP4__ATOM_TYPE_FTYP MP4__ATOM_BODY_FTYP
#define MP4_ATOM_FREE MP4__ATOM_TYPE_FREE
#define MP4_ATOM_MOOV MP4__ATOM_TYPE_MOOV MP4__ATOM_BODY_MOOV
#define MP4_ATOM_MDAT MP4__ATOM_TYPE_MDAT MP4__ATOM_BODY_MDAT

#define SRC_READ_BUF_SZ 15

#define EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG       (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define EXPECT_FCHUNK_SEQ_INDEX__IN_ASA_USRARG    (ASAMAP_INDEX__IN_ASA_USRARG + 2)
#define EXPECT_MDAT_POS_INDEX__IN_ASA_USRARG      (ASAMAP_INDEX__IN_ASA_USRARG + 3)
#define EXPECT_STREAMINFO_SZ_INDEX__IN_ASA_USRARG (ASAMAP_INDEX__IN_ASA_USRARG + 4)
#define DONE_FLAG_INDEX__IN_ASA_USRARG            (ASAMAP_INDEX__IN_ASA_USRARG + 5)
#define NUM_CB_ARGS_ASAOBJ                        (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)

static void mock_mp4_asa_src_open_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result) {
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
}

static __attribute__((optimize("O0"))) void
utest_init_mp4_preload(atfp_mp4_t *mp4proc, void (*prepare_fchunk_fn)(atfp_mp4_t *)) {
    app_envvars_t env = {0};
    app_load_envvars(&env);
    atfp_t *processor = &mp4proc->super;
    processor->data.error = json_object();
    processor->data.spec = json_object();
    json_object_set_new(processor->data.spec, "parts_size", json_array());
    uv_loop_t            *loop = uv_default_loop();
    atfp_asa_map_t       *map = atfp_asa_map_init(1);
    asa_cfg_t            *src_storage = calloc(1, sizeof(asa_cfg_t));
    asa_op_localfs_cfg_t *asa_local_cfg = calloc(1, sizeof(asa_op_localfs_cfg_t));
    asa_op_localfs_cfg_t *asa_src_cfg = calloc(1, sizeof(asa_op_localfs_cfg_t));
    processor->data.storage.handle = &asa_src_cfg->super;
    processor->data.storage.basepath = strdup(PRELOAD_SRC_BASEPATH);
    src_storage->ops = (asa_cfg_ops_t
    ){.fn_open = app_storage_localfs_open,
      .fn_close = app_storage_localfs_close,
      .fn_read = app_storage_localfs_read,
      .fn_write = app_storage_localfs_write};
    src_storage->base_path = env.sys_base_path;
    asa_src_cfg->loop = asa_local_cfg->loop = loop;
    asa_src_cfg->super.storage = src_storage;
    asa_src_cfg->super.cb_args.size = NUM_CB_ARGS_ASAOBJ;
    asa_src_cfg->super.cb_args.entries = calloc(NUM_CB_ARGS_ASAOBJ, sizeof(void *));
    asa_src_cfg->super.cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = mp4proc;
    asa_src_cfg->super.op.read.dst = malloc(SRC_READ_BUF_SZ * sizeof(char));
    asa_src_cfg->super.op.read.dst_max_nbytes = SRC_READ_BUF_SZ;
    asa_local_cfg->super.storage = src_storage;
    asa_local_cfg->super.cb_args.size = ASAMAP_INDEX__IN_ASA_USRARG + 1;
    asa_local_cfg->super.cb_args.entries = calloc(ASAMAP_INDEX__IN_ASA_USRARG + 1, sizeof(void *));
    atfp_asa_map_set_source(map, &asa_src_cfg->super);
    atfp_asa_map_set_localtmp(map, asa_local_cfg);
    { //  create source file chunks for tests
        PATH_CONCAT_THEN_RUN(env.sys_base_path, UNITTEST_FULLPATH, RUNNER_CREATE_FOLDER);
        PATH_CONCAT_THEN_RUN(env.sys_base_path, PRELOAD_BASEPATH, RUNNER_CREATE_FOLDER);
        PATH_CONCAT_THEN_RUN(env.sys_base_path, PRELOAD_SRC_BASEPATH, RUNNER_CREATE_FOLDER);
        prepare_fchunk_fn(mp4proc);
    }
    { // open first chunk of the source
        ASA_RES_CODE result =
            atfp_open_srcfile_chunk(&asa_src_cfg->super, PRELOAD_SRC_BASEPATH, 1, mock_mp4_asa_src_open_cb);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if (result == ASTORAGE_RESULT_ACCEPT)
            uv_run(loop, UV_RUN_ONCE);
    }
    { // open local temp buffer
        asa_local_cfg->super.op.open.cb = mock_mp4_asa_src_open_cb;
        asa_local_cfg->super.op.open.mode = S_IRUSR | S_IWUSR;
        asa_local_cfg->super.op.open.flags = O_RDWR | O_CREAT;
        asa_local_cfg->super.op.open.dst_path = strdup(LOCAL_TMPBUF_PATH);
        ASA_RES_CODE result = app_storage_localfs_open(&asa_local_cfg->super);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if (result == ASTORAGE_RESULT_ACCEPT)
            uv_run(loop, UV_RUN_ONCE);
    }
} // end of utest_init_mp4_preload

static __attribute__((optimize("O0"))) void utest_deinit_mp4_preload(atfp_mp4_t *mp4proc) {
    atfp_t               *processor = &mp4proc->super;
    asa_op_localfs_cfg_t *asa_src_cfg = (asa_op_localfs_cfg_t *)processor->data.storage.handle;
    atfp_asa_map_t       *map = asa_src_cfg->super.cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local_cfg = atfp_asa_map_get_localtmp(map);

    asa_cfg_t  *src_storage = asa_src_cfg->super.storage;
    const char *sys_basepath = src_storage->base_path;
    if (asa_src_cfg) {
        if (asa_src_cfg->super.cb_args.entries) {
            free(asa_src_cfg->super.cb_args.entries);
            asa_src_cfg->super.cb_args.entries = NULL;
        }
        if (asa_src_cfg->super.op.read.dst) {
            free(asa_src_cfg->super.op.read.dst);
            asa_src_cfg->super.op.read.dst = NULL;
        }
        if (asa_src_cfg->super.op.open.dst_path) {
            free(asa_src_cfg->super.op.open.dst_path);
            asa_src_cfg->super.op.open.dst_path = NULL;
        }
        free(asa_src_cfg);
        processor->data.storage.handle = NULL;
    }
    if (asa_local_cfg) {
        if (asa_local_cfg->super.cb_args.entries) {
            free(asa_local_cfg->super.cb_args.entries);
            asa_local_cfg->super.cb_args.entries = NULL;
        }
        if (asa_local_cfg->super.op.open.dst_path) {
            free(asa_local_cfg->super.op.open.dst_path);
            asa_local_cfg->super.op.open.dst_path = NULL;
        }
        free(asa_local_cfg);
    }
    atfp_asa_map_deinit(map);
    if (src_storage) {
        free(src_storage);
    }
    if (processor->data.storage.basepath) {
        free((char *)processor->data.storage.basepath);
        processor->data.storage.basepath = NULL;
    }
    if (processor->data.error) {
        json_decref(processor->data.error);
        processor->data.error = NULL;
    }
    if (processor->data.spec) {
        json_decref(processor->data.spec);
        processor->data.spec = NULL;
    }
    {
#define RUNNER_SCANDIR(fullpath) scandir(fullpath, &namelist, NULL, alphasort)

#define PRELOAD_SRCFILE_TEMPLATE PRELOAD_SRC_BASEPATH "/%s"
        struct dirent **namelist = NULL;
        int             n = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH, RUNNER_SCANDIR);
        for (int idx = 0; idx < n; idx++) {
            // printf("%s\n", namelist[idx]->d_name);
            if (namelist[idx]->d_name[0] != '.') {
                size_t sz = sizeof(PRELOAD_SRCFILE_TEMPLATE);
                char   path[sz];
                int    nwrite = snprintf(&path[0], sz, PRELOAD_SRCFILE_TEMPLATE, namelist[idx]->d_name);
                path[nwrite++] = 0x0;
                // printf("%s\n", &path[0]);
                assert(sz >= nwrite); // Ensure that the snprintf operation didn't truncate the path
                PATH_CONCAT_THEN_RUN(sys_basepath, &path[0], unlink);
            }
            free(namelist[idx]);
        }
        free(namelist);
    }
    PATH_CONCAT_THEN_RUN(sys_basepath, LOCAL_TMPBUF_PATH, unlink);
#undef PRELOAD_SRCFILE_TEMPLATE
    PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH, rmdir);
    PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_BASEPATH, rmdir);
    PATH_CONCAT_THEN_RUN(sys_basepath, UNITTEST_FULLPATH, rmdir);
} // end of utest_deinit_mp4_preload

static void utest_atfp_mp4__preload_stream_info__done_ok(atfp_mp4_t *mp4proc) {
    atfp_t *processor = &mp4proc->super;
    json_t *err_info = processor->data.error;
    assert_that(json_object_size(err_info), is_equal_to(0));
    asa_op_base_cfg_t    *asa_src_cfg = processor->data.storage.handle;
    atfp_asa_map_t       *map = asa_src_cfg->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local_cfg = atfp_asa_map_get_localtmp(map);
    assert_that(json_object_size(err_info), is_equal_to(0));
    if (json_object_size(err_info) == 0) {
        // verify the sequence of preloaded atoms, which should be :
        // ftype --> free(optional) --> moov --> mdat
        int  nread = 0;
        int  local_tmppbuf_fd = asa_local_cfg->file.file;
        char actual_content[sizeof(MP4__ATOM_BODY_MDAT)] = {0};
        lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_SET);
        read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_TYPE_FTYP));
        read(local_tmppbuf_fd, &actual_content[0], strlen(MP4__ATOM_BODY_FTYP));
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_BODY_FTYP));
        lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_CUR);
        nread = read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
        actual_content[nread] = 0x0;
        if (!strncmp(MP4__ATOM_TYPE_FREE, &actual_content[0], nread)) {
            assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_TYPE_FREE));
            lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_CUR);
            nread = read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
            actual_content[nread] = 0x0;
        }
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_TYPE_MOOV));
        nread = read(local_tmppbuf_fd, &actual_content[0], strlen(MP4__ATOM_BODY_MOOV));
        actual_content[nread] = 0x0;
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_BODY_MOOV));
        lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_CUR);
        nread = read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
        actual_content[nread] = 0x0;
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_TYPE_MDAT));
        nread = read(local_tmppbuf_fd, &actual_content[0], 1);
        assert_that(nread, is_equal_to(0));
        {
            uint32_t expect_mdat_sz =
                *(uint32_t *)asa_src_cfg->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG];
            uint32_t actual_mdat_sz = mp4proc->internal.mdat.size;
            assert_that(actual_mdat_sz, is_equal_to(expect_mdat_sz));
            uint32_t expect_fchunk_seq =
                *(uint32_t *)asa_src_cfg->cb_args.entries[EXPECT_FCHUNK_SEQ_INDEX__IN_ASA_USRARG];
            uint32_t actual_fchunk_seq = mp4proc->internal.mdat.fchunk_seq;
            assert_that(actual_fchunk_seq, is_equal_to(expect_fchunk_seq));
            uint32_t expect_mdat_pos =
                *(uint32_t *)asa_src_cfg->cb_args.entries[EXPECT_MDAT_POS_INDEX__IN_ASA_USRARG];
            uint32_t actual_mdat_pos = mp4proc->internal.mdat.pos;
            assert_that(expect_mdat_pos, is_equal_to(actual_mdat_pos));
        }
        {
            uint32_t expect_preload_sz =
                *(uint32_t *)asa_src_cfg->cb_args.entries[EXPECT_STREAMINFO_SZ_INDEX__IN_ASA_USRARG];
            assert_that(mp4proc->internal.preload_pkts.size, is_equal_to((size_t)expect_preload_sz));
            assert_that(mp4proc->internal.preload_pkts.nbytes_copied, is_equal_to(expect_preload_sz));
        }
    } // end of err_info is empty
    uint8_t *done_flag = asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    *done_flag = 1;
} // end of utest_atfp_mp4__preload_stream_info__done_ok

static void utest_atfp_mp4__preload_packet__done_ok(atfp_mp4_t *mp4proc) {
    atfp_t            *processor = &mp4proc->super;
    json_t            *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src_cfg = processor->data.storage.handle;
    assert_that(json_object_size(err_info), is_equal_to(0));
    uint8_t *done_flag = asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    *done_flag = 1;
} // end of utest_atfp_mp4__preload_packet__done_ok

#define NUM_FILE_CHUNKS 4
static void preload_stream_info_ok_1__prepare_fchunks(atfp_mp4_t *mp4proc) {
    int     fds[NUM_FILE_CHUNKS] = {0};
    atfp_t *processor = &mp4proc->super;

    asa_op_localfs_cfg_t *asa_src_cfg = (asa_op_localfs_cfg_t *)processor->data.storage.handle;
    asa_cfg_t            *src_storage = asa_src_cfg->super.storage;
    const char           *sys_basepath = src_storage->base_path;
    fds[0] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/1", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[1] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/2", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[2] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/3", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[3] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/4", RUNNER_OPEN_WRONLY_CREATE_USR);
    uint32_t ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t free_sz = strlen(MP4_ATOM_FREE) + sizeof(uint32_t);
    uint32_t moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t free_sz_be = htobe32(free_sz);
    uint32_t moov_sz_be = htobe32(moov_sz);
    uint32_t mdat_sz_be = htobe32(mdat_sz);
    write(fds[0], (char *)&ftyp_sz_be, sizeof(uint32_t));
    write(fds[0], MP4_ATOM_FTYP, ftyp_sz - sizeof(uint32_t));
    write(fds[0], (char *)&free_sz_be, sizeof(uint32_t));
    write(fds[1], MP4_ATOM_FREE, free_sz - sizeof(uint32_t));
    write(fds[1], (char *)&moov_sz_be, sizeof(uint32_t));
    write(fds[1], MP4_ATOM_MOOV, moov_sz - sizeof(uint32_t));
    write(fds[1], (char *)&mdat_sz_be, sizeof(uint32_t));
    write(fds[2], MP4__ATOM_TYPE_MDAT, strlen(MP4__ATOM_TYPE_MDAT));
    uint32_t mdat_body_rd_offset = lseek(fds[2], 0, SEEK_CUR);
    {
        const char *mdat_body_ptr = MP4__ATOM_BODY_MDAT;
        char       *ptr = mdat_body_ptr;
        size_t      sz = strlen(mdat_body_ptr);
        write(fds[2], ptr, 3);
        ptr += 3;
        sz -= 3;
        write(fds[3], ptr, sz);
    }
    {
        asa_op_base_cfg_t *asa_src_cfg = mp4proc->super.data.storage.handle;
        uint32_t          *mdat_info = calloc(4, sizeof(uint32_t));
        mdat_info[0] = strlen(MP4__ATOM_BODY_MDAT);
        mdat_info[1] = 2;
        mdat_info[2] = mdat_body_rd_offset;
        mdat_info[3] = ftyp_sz + free_sz + moov_sz + strlen(MP4__ATOM_TYPE_MDAT) + sizeof(uint32_t);
        asa_src_cfg->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG] =
            &mdat_info[0]; // size of mdat body
        asa_src_cfg->cb_args.entries[EXPECT_FCHUNK_SEQ_INDEX__IN_ASA_USRARG] =
            &mdat_info[1]; // sequemce number of file chunk where mdat body starts
        asa_src_cfg->cb_args.entries[EXPECT_MDAT_POS_INDEX__IN_ASA_USRARG] =
            &mdat_info[2]; // offset of the file chunk
        asa_src_cfg->cb_args.entries[EXPECT_STREAMINFO_SZ_INDEX__IN_ASA_USRARG] = &mdat_info[3];
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for (int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer(lseek(fds[idx], 0, SEEK_CUR)));
        close(fds[idx]);
    }
} // end of preload_stream_info_ok_1__prepare_fchunks

Ensure(atfp_mp4_test__preload_stream_info_ok_1) { // `mdat` comes after `moov`
    atfp_mp4_t mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_ok_1__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.storage.handle;
    uv_loop_t         *loop = uv_default_loop();
    ASA_RES_CODE       result =
        atfp_mp4__preload_stream_info(&mp4proc, utest_atfp_mp4__preload_stream_info__done_ok);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t done_flag = 0;
    asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag;
    while (!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    free(asa_src_cfg->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG]);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info_ok_1
#undef NUM_FILE_CHUNKS

#define NUM_FILE_CHUNKS 6
static void preload_stream_info_ok_2__prepare_fchunks(atfp_mp4_t *mp4proc) {
    int     fds[NUM_FILE_CHUNKS] = {0};
    atfp_t *processor = &mp4proc->super;

    asa_op_localfs_cfg_t *asa_src_cfg = (asa_op_localfs_cfg_t *)processor->data.storage.handle;
    asa_cfg_t            *src_storage = asa_src_cfg->super.storage;
    const char           *sys_basepath = src_storage->base_path;
    fds[0] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/1", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[1] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/2", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[2] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/3", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[3] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/4", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[4] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/5", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[5] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/6", RUNNER_OPEN_WRONLY_CREATE_USR);
    uint32_t ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t moov_sz_be = htobe32(moov_sz);
    uint32_t mdat_sz_be = htobe32(mdat_sz);
    write(fds[0], (char *)&ftyp_sz_be, sizeof(uint32_t));
    write(fds[0], MP4_ATOM_FTYP, ftyp_sz - sizeof(uint32_t));
    uint32_t mdat_body_rd_offset = 0;
    { // mdat
        write(fds[0], (char *)&mdat_sz_be, sizeof(uint32_t));
        write(fds[1], MP4__ATOM_TYPE_MDAT, sizeof(uint32_t));
        mdat_body_rd_offset = lseek(fds[1], 0, SEEK_CUR);
        uint32_t    body_tot_sz = strlen(MP4__ATOM_BODY_MDAT);
        const char *body_ptr = MP4__ATOM_BODY_MDAT;
#define MP4__ATOM_BODY_MDAT__FIRST_PIECE__SZ 11
        write(fds[1], body_ptr, MP4__ATOM_BODY_MDAT__FIRST_PIECE__SZ);
        body_ptr += MP4__ATOM_BODY_MDAT__FIRST_PIECE__SZ;
        body_tot_sz -= MP4__ATOM_BODY_MDAT__FIRST_PIECE__SZ;
        write(fds[2], body_ptr, 13);
        body_ptr += 13;
        body_tot_sz -= 13;
        write(fds[3], body_ptr, body_tot_sz);
    }
    { // moov
        char *moov_sz_be_str = (char *)&moov_sz_be;
        write(fds[3], &moov_sz_be_str[0], 3);
        write(fds[4], &moov_sz_be_str[3], 1);
        write(fds[4], MP4__ATOM_TYPE_MOOV, sizeof(uint32_t));
        uint32_t    body_tot_sz = strlen(MP4__ATOM_BODY_MOOV);
        const char *body_ptr = MP4__ATOM_BODY_MOOV;
        write(fds[4], body_ptr, 7);
        body_ptr += 7;
        body_tot_sz -= 7;
        write(fds[5], body_ptr, body_tot_sz);
    }
    {
        asa_op_base_cfg_t *asa_src_cfg = mp4proc->super.data.storage.handle;
        uint32_t          *mdat_info = calloc(4, sizeof(uint32_t));
        mdat_info[0] = strlen(MP4__ATOM_BODY_MDAT);
        mdat_info[1] = 1;
        mdat_info[2] = mdat_body_rd_offset;
        mdat_info[3] = ftyp_sz + moov_sz + strlen(MP4__ATOM_TYPE_MDAT) + sizeof(uint32_t);
        asa_src_cfg->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG] =
            &mdat_info[0]; // size of mdat body
        asa_src_cfg->cb_args.entries[EXPECT_FCHUNK_SEQ_INDEX__IN_ASA_USRARG] =
            &mdat_info[1]; // sequemce number of file chunk where mdat body starts
        asa_src_cfg->cb_args.entries[EXPECT_MDAT_POS_INDEX__IN_ASA_USRARG] =
            &mdat_info[2]; // offset of the file chunk
        asa_src_cfg->cb_args.entries[EXPECT_STREAMINFO_SZ_INDEX__IN_ASA_USRARG] = &mdat_info[3];
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for (int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer(lseek(fds[idx], 0, SEEK_CUR)));
        close(fds[idx]);
    }
} // end of preload_stream_info_ok_2__prepare_fchunks

Ensure(atfp_mp4_test__preload_stream_info_ok_2) { // `mdat` comes before `moov`
    atfp_mp4_t mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_ok_2__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.storage.handle;
    uv_loop_t         *loop = uv_default_loop();
    ASA_RES_CODE       result =
        atfp_mp4__preload_stream_info(&mp4proc, utest_atfp_mp4__preload_stream_info__done_ok);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t done_flag = 0;
    asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag;
    while (!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    free(asa_src_cfg->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG]);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info_ok_2
#undef NUM_FILE_CHUNKS

static void utest_atfp_mp4__preload_stream_info__done_error(atfp_mp4_t *mp4proc) {
    atfp_t *processor = &mp4proc->super;
    json_t *err_info = processor->data.error;
    assert_that(json_object_size(err_info), is_greater_than(0));
    asa_op_base_cfg_t *asa_src_cfg = processor->data.storage.handle;
    uint8_t           *done_flag = asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    *done_flag = 1;
}

#define NUM_FILE_CHUNKS 3
static void preload_stream_info_corrupt_moov__prepare_fchunks(atfp_mp4_t *mp4proc) {
    int     fds[NUM_FILE_CHUNKS] = {0};
    atfp_t *processor = &mp4proc->super;

    asa_op_localfs_cfg_t *asa_src_cfg = (asa_op_localfs_cfg_t *)processor->data.storage.handle;
    asa_cfg_t            *src_storage = asa_src_cfg->super.storage;
    const char           *sys_basepath = src_storage->base_path;
    fds[0] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/1", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[1] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/2", RUNNER_OPEN_WRONLY_CREATE_USR);
    fds[2] = PATH_CONCAT_THEN_RUN(sys_basepath, PRELOAD_SRC_BASEPATH "/3", RUNNER_OPEN_WRONLY_CREATE_USR);
    uint32_t ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t moov_sz_be = htobe32(moov_sz);
    uint32_t mdat_sz_be = htobe32(mdat_sz);
    {
        write(fds[0], (char *)&ftyp_sz_be, sizeof(uint32_t));
        write(fds[0], MP4_ATOM_FTYP, ftyp_sz - sizeof(uint32_t));
        write(fds[1], (char *)&moov_sz_be, sizeof(uint32_t));
        write(fds[1], MP4_ATOM_MOOV, moov_sz - sizeof(uint32_t) - 1); // corruption starts at here
        write(fds[2], (char *)&mdat_sz_be, sizeof(uint32_t));
        write(fds[2], MP4_ATOM_MDAT, mdat_sz - sizeof(uint32_t));
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for (int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer(lseek(fds[idx], 0, SEEK_CUR)));
        close(fds[idx]);
    }
} // end of preload_stream_info_corrupt_moov__prepare_fchunks

Ensure(atfp_mp4_test__preload_stream_info__corrupted_moov) {
    atfp_mp4_t mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_corrupt_moov__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.storage.handle;
    uv_loop_t         *loop = uv_default_loop();
    ASA_RES_CODE       result =
        atfp_mp4__preload_stream_info(&mp4proc, utest_atfp_mp4__preload_stream_info__done_error);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t done_flag = 0;
    asa_src_cfg->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag;
    while (!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info__corrupted_moov
#undef NUM_FILE_CHUNKS

Ensure(atfp_mp4_test__preload_mdat_packet_ok) {
    atfp_mp4_t mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_ok_2__prepare_fchunks);
    asa_op_base_cfg_t    *asa_src = mp4proc.super.data.storage.handle;
    atfp_asa_map_t       *map = asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(map);
    int                   local_tmppbuf_fd = asa_local->file.file;
    uv_loop_t            *loop = uv_default_loop();
    uint8_t               done_flag = 0;
    asa_src->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag;
    int    chunk_idx_start = 0;
    size_t chunk_offset = 0, nbytes_to_load = 0;
    { // subcase #1
        chunk_idx_start = *(uint32_t *)asa_src->cb_args.entries[EXPECT_FCHUNK_SEQ_INDEX__IN_ASA_USRARG];
        chunk_offset = *(uint32_t *)asa_src->cb_args.entries[EXPECT_MDAT_POS_INDEX__IN_ASA_USRARG];
        nbytes_to_load = *(uint32_t *)asa_src->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG];
        ASA_RES_CODE result = atfp_mp4__preload_packet_sequence(
            &mp4proc, chunk_idx_start, chunk_offset, nbytes_to_load, utest_atfp_mp4__preload_packet__done_ok
        );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        for (done_flag = 0; !done_flag;)
            uv_run(loop, UV_RUN_ONCE);
        char actual_content[sizeof(MP4__ATOM_BODY_MDAT)] = {0};
        lseek(local_tmppbuf_fd, 0, SEEK_SET);
        read(local_tmppbuf_fd, &actual_content[0], sizeof(MP4__ATOM_BODY_MDAT));
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_BODY_MDAT));
    }
    { // subcase #2
        chunk_idx_start += 1;
        chunk_offset = 1;
        nbytes_to_load = 10;
        lseek(local_tmppbuf_fd, 0, SEEK_SET);
        ASA_RES_CODE result = atfp_mp4__preload_packet_sequence(
            &mp4proc, chunk_idx_start, chunk_offset, nbytes_to_load, utest_atfp_mp4__preload_packet__done_ok
        );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        for (done_flag = 0; !done_flag;)
            uv_run(loop, UV_RUN_ONCE);
        char expect_content[nbytes_to_load + 1];
        char actual_content[nbytes_to_load + 1];
        expect_content[nbytes_to_load] = 0x0;
        actual_content[nbytes_to_load] = 0x0;
        lseek(local_tmppbuf_fd, 0, SEEK_SET);
        read(local_tmppbuf_fd, &actual_content[0], nbytes_to_load);
        const char *mdat_body = MP4__ATOM_BODY_MDAT;
        memcpy(
            &expect_content[0], &mdat_body[MP4__ATOM_BODY_MDAT__FIRST_PIECE__SZ + chunk_offset],
            nbytes_to_load
        );
        assert_that(&actual_content[0], is_equal_to_string(&expect_content[0]));
    }
    free(asa_src->cb_args.entries[EXPECT_MDAT_SZ_INDEX__IN_ASA_USRARG]);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_mdat_packet_ok

TestSuite *app_transcoder_mp4_preload_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_mp4_test__preload_stream_info_ok_1);
    add_test(suite, atfp_mp4_test__preload_stream_info_ok_2);
    add_test(suite, atfp_mp4_test__preload_stream_info__corrupted_moov);
    add_test(suite, atfp_mp4_test__preload_mdat_packet_ok);
    return suite;
}
