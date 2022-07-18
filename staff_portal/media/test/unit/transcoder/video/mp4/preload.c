#include <assert.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <uv.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>

#include "transcoder/video/mp4.h"

#define  LOCAL_TMPBUF_BASEPATH  "tmp/buffer/media/test"
#define  UNITTEST_FOLDER_NAME   "utest"
#define  PRELOAD_FOLDER_NAME    "mock_preload"
#define  PRELOAD_SRCFILE_FOLDER_NAME    "src"
#define  LOCAL_TMPBUF_NAME      "local_tmpbuf"

#define  UNITTEST_FULLPATH      LOCAL_TMPBUF_BASEPATH "/"  UNITTEST_FOLDER_NAME
#define  PRELOAD_BASEPATH       UNITTEST_FULLPATH     "/"  PRELOAD_FOLDER_NAME
#define  PRELOAD_SRC_BASEPATH   PRELOAD_BASEPATH   "/"  PRELOAD_SRCFILE_FOLDER_NAME
#define  LOCAL_TMPBUF_PATH      PRELOAD_BASEPATH   "/"  LOCAL_TMPBUF_NAME

#define  MP4__ATOM_TYPE_FTYP   "ftyp"
#define  MP4__ATOM_TYPE_FREE   "free"
#define  MP4__ATOM_TYPE_MOOV   "moov"
#define  MP4__ATOM_TYPE_MDAT   "mdat"

#define  MP4__ATOM_BODY_FTYP   "interchangeable_receiver"
#define  MP4__ATOM_BODY_MOOV   "assembly_line_worker_progress_millions_of_mountains"
#define  MP4__ATOM_BODY_MDAT   "loose_coupling_between_app_server_and_SQLdatabase_&_nosql_storage"

#define  MP4_ATOM_FTYP   MP4__ATOM_TYPE_FTYP  MP4__ATOM_BODY_FTYP
#define  MP4_ATOM_FREE   MP4__ATOM_TYPE_FREE
#define  MP4_ATOM_MOOV   MP4__ATOM_TYPE_MOOV  MP4__ATOM_BODY_MOOV
#define  MP4_ATOM_MDAT   MP4__ATOM_TYPE_MDAT  MP4__ATOM_BODY_MDAT

#define  SRC_READ_BUF_SZ  15

static void  mock_mp4_asa_src_open_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
}
  
static __attribute__((optimize("O0"))) void  utest_init_mp4_preload(atfp_mp4_t *mp4proc, void  (*prepare_fchunk_fn)(atfp_mp4_t *))
{
    atfp_t *processor = & mp4proc -> super;
    processor->data.error = json_object();
    processor->data.spec  = json_object();
    json_object_set_new(processor->data.spec, "parts_size", json_array());
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  *asa_local_cfg = &mp4proc->local_tmpbuf_handle;
    asa_op_localfs_cfg_t  *asa_src_cfg = calloc(1, sizeof(asa_op_localfs_cfg_t));
    asa_cfg_t  *src_storage = calloc(1, sizeof(asa_cfg_t));
    processor->data.src.storage.handle = &asa_src_cfg->super;
    processor->data.src.storage.config = src_storage;
    processor->data.src.basepath = strdup(PRELOAD_SRC_BASEPATH);
    src_storage->ops = (asa_cfg_ops_t) {
        .fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close,
        .fn_read=app_storage_localfs_read 
    };
    asa_src_cfg->loop = asa_local_cfg->loop = processor->data.loop = loop;
    asa_src_cfg->super.cb_args.size = 5;
    asa_src_cfg->super.cb_args.entries = calloc(5, sizeof(void *));
    asa_src_cfg->super.cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG] = mp4proc;
    asa_src_cfg->super.cb_args.entries[1] = NULL;
    asa_src_cfg->super.op.read.dst = malloc(SRC_READ_BUF_SZ * sizeof(char));
    asa_src_cfg->super.op.read.dst_max_nbytes = SRC_READ_BUF_SZ;
    { //  create source file chunks for tests
        mkdir(UNITTEST_FULLPATH, S_IRWXU);
        mkdir(PRELOAD_BASEPATH, S_IRWXU);
        mkdir(PRELOAD_SRC_BASEPATH, S_IRWXU);
        prepare_fchunk_fn(mp4proc);
    } { // open first chunk of the source
        ASA_RES_CODE result = atfp_open_srcfile_chunk( &asa_src_cfg->super, src_storage,
             PRELOAD_SRC_BASEPATH, 1, mock_mp4_asa_src_open_cb);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_ACCEPT)
            uv_run(processor->data.loop, UV_RUN_ONCE);
    } { // open local temp buffer
        asa_local_cfg->super.op.open.cb = mock_mp4_asa_src_open_cb;
        asa_local_cfg->super.op.open.mode  = S_IRUSR | S_IWUSR;
        asa_local_cfg->super.op.open.flags = O_RDWR | O_CREAT;
        asa_local_cfg->super.op.open.dst_path = strdup(LOCAL_TMPBUF_PATH);
        ASA_RES_CODE result = app_storage_localfs_open(&asa_local_cfg->super);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_ACCEPT)
            uv_run(processor->data.loop, UV_RUN_ONCE);
    }
} // end of utest_init_mp4_preload


static __attribute__((optimize("O0"))) void  utest_deinit_mp4_preload(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = & mp4proc -> super;
    asa_op_localfs_cfg_t  *asa_local_cfg = &mp4proc->local_tmpbuf_handle;
    asa_op_localfs_cfg_t  *asa_src_cfg = (asa_op_localfs_cfg_t *) processor->data.src.storage.handle;
    asa_cfg_t  *src_storage = processor->data.src.storage.config;
    if(asa_src_cfg) {
        if(asa_src_cfg->super.cb_args.entries) {
            free(asa_src_cfg->super.cb_args.entries);
            asa_src_cfg->super.cb_args.entries = NULL;
        }
        if(asa_src_cfg->super.op.read.dst) {
            free(asa_src_cfg->super.op.read.dst);
            asa_src_cfg->super.op.read.dst = NULL;
        }
        free(asa_src_cfg);
        processor->data.src.storage.handle = NULL;
    }
    if(src_storage) {
        free(src_storage);
        processor->data.src.storage.config = NULL;
    }
    if(processor->data.src.basepath) {
        free((char *)processor->data.src.basepath);
        processor->data.src.basepath = NULL;
    }
    if(asa_local_cfg->super.op.open.dst_path) {
        free(asa_local_cfg->super.op.open.dst_path);
        asa_local_cfg->super.op.open.dst_path = NULL;
    }
    if(processor->data.error) {
        json_decref(processor->data.error);
        processor->data.error = NULL;
    }
    if(processor->data.spec) {
        json_decref(processor->data.spec);
        processor->data.spec = NULL;
    }
    if(1) {}
    if(1) {}
    if(1) {}
    {
#define  PRELOAD_SRCFILE_TEMPLATE   PRELOAD_SRC_BASEPATH  "/%s"
        struct dirent **namelist = NULL;
        int n = scandir(PRELOAD_SRC_BASEPATH, &namelist, NULL, alphasort);
        for(int idx = 0; idx < n; idx++) {
            // printf("%s\n", namelist[idx]->d_name);
            if(namelist[idx]->d_name[0] != '.') {
                size_t sz = sizeof(PRELOAD_SRCFILE_TEMPLATE);
                char path[sz];
                int nwrite = snprintf(&path[0], sz, PRELOAD_SRCFILE_TEMPLATE, namelist[idx]->d_name);
                path[nwrite++] = 0x0;
                //printf("%s\n", &path[0]);
                assert(sz >= nwrite);
                unlink(&path[0]);
            }
            free(namelist[idx]);
        }
#undef  PRELOAD_SRCFILE_TEMPLATE
    }
    unlink(LOCAL_TMPBUF_PATH);
    rmdir(PRELOAD_SRC_BASEPATH);
    rmdir(PRELOAD_BASEPATH);
    rmdir(UNITTEST_FULLPATH);
} // end of utest_deinit_mp4_preload


static void utest_atfp_mp4__preload_stream_info__done_ok(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info  = processor->data.error;
    assert_that(json_object_size(err_info), is_equal_to(0));
    asa_op_base_cfg_t     *asa_src_cfg   =  processor->data.src.storage.handle;
    asa_op_localfs_cfg_t  *asa_local_cfg = &mp4proc->local_tmpbuf_handle;
    if(json_object_size(err_info) == 0) {
        // verify the sequence of preloaded atoms, which should be :
        // ftype --> free(optional) --> moov --> mdat
        int nread = 0;
        int local_tmppbuf_fd = asa_local_cfg->file.file;
        char actual_content[sizeof(MP4__ATOM_BODY_MDAT)] = {0};
        lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_SET);
        read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_TYPE_FTYP));
        read(local_tmppbuf_fd, &actual_content[0], strlen(MP4__ATOM_BODY_FTYP));
        assert_that(&actual_content[0], is_equal_to_string(MP4__ATOM_BODY_FTYP));
        lseek(local_tmppbuf_fd, sizeof(uint32_t), SEEK_CUR);
        nread = read(local_tmppbuf_fd, &actual_content[0], sizeof(uint32_t));
        actual_content[nread] = 0x0;
        if(!strncmp(MP4__ATOM_TYPE_FREE, &actual_content[0], nread)) {
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
            uint32_t expect_mdat_sz = *(uint32_t*) asa_src_cfg->cb_args.entries[2];
            uint32_t actual_mdat_sz =  mp4proc->internal.mdat.size;
            assert_that(actual_mdat_sz, is_equal_to(expect_mdat_sz));
            uint32_t expect_fchunk_seq = *(uint32_t*) asa_src_cfg->cb_args.entries[3];
            uint32_t actual_fchunk_seq =  mp4proc->internal.mdat.fchunk_seq;
            assert_that(actual_fchunk_seq, is_equal_to(expect_fchunk_seq));
            uint32_t expect_mdat_pos = *(uint32_t*) asa_src_cfg->cb_args.entries[4];
            uint32_t actual_mdat_pos =  mp4proc->internal.mdat.pos;
            assert_that(expect_mdat_pos, is_equal_to(actual_mdat_pos));
        }
    } // end of err_info is empty
    uint8_t *done_flag = asa_src_cfg->cb_args.entries[1];
    *done_flag = 1;
} // end of utest_atfp_mp4__preload_stream_info__done_ok


#define  NUM_FILE_CHUNKS  4
static void preload_stream_info_ok_1__prepare_fchunks (atfp_mp4_t *mp4proc)
{
    int fds[NUM_FILE_CHUNKS] = {0};
    fds[0] = open(PRELOAD_SRC_BASEPATH "/1", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[1] = open(PRELOAD_SRC_BASEPATH "/2", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[2] = open(PRELOAD_SRC_BASEPATH "/3", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[3] = open(PRELOAD_SRC_BASEPATH "/4", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    uint32_t  ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t  free_sz = strlen(MP4_ATOM_FREE) + sizeof(uint32_t);
    uint32_t  moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t  mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t  ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t  free_sz_be = htobe32(free_sz);
    uint32_t  moov_sz_be = htobe32(moov_sz);
    uint32_t  mdat_sz_be = htobe32(mdat_sz);
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
        char *ptr = mdat_body_ptr;
        size_t sz = strlen(mdat_body_ptr);
        write(fds[2], ptr, 3);
        ptr += 3;
        sz  -= 3;
        write(fds[3], ptr, sz);
    } {
        asa_op_base_cfg_t *asa_src_cfg = mp4proc->super.data.src.storage.handle;
        uint32_t *mdat_info = calloc(3, sizeof(uint32_t));
        mdat_info[0] = strlen(MP4__ATOM_BODY_MDAT);
        mdat_info[1] = 2;
        mdat_info[2] = mdat_body_rd_offset;
        asa_src_cfg->cb_args.entries[2] = &mdat_info[0]; // size of mdat body
        asa_src_cfg->cb_args.entries[3] = &mdat_info[1]; // sequemce number of file chunk where mdat body starts
        asa_src_cfg->cb_args.entries[4] = &mdat_info[2]; // offset of the file chunk
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for(int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer( lseek(fds[idx], 0, SEEK_CUR) ));
        close(fds[idx]);
    }
} // end of preload_stream_info_ok_1__prepare_fchunks


Ensure(atfp_mp4_test__preload_stream_info_ok_1)
{ // `mdat` comes after `moov`
    atfp_mp4_t  mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_ok_1__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.src.storage.handle;
    uv_loop_t  *loop = mp4proc.super.data.loop ;
    ASA_RES_CODE  result = atfp_mp4__preload_stream_info( &mp4proc,
            utest_atfp_mp4__preload_stream_info__done_ok );
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t  done_flag = 0;
    asa_src_cfg->cb_args.entries[1] = &done_flag;
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    free(asa_src_cfg->cb_args.entries[2]);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info_ok_1
#undef  NUM_FILE_CHUNKS


#define  NUM_FILE_CHUNKS  6
static void preload_stream_info_ok_2__prepare_fchunks (atfp_mp4_t *mp4proc)
{
    int fds[NUM_FILE_CHUNKS] = {0};
    fds[0] = open(PRELOAD_SRC_BASEPATH "/1", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[1] = open(PRELOAD_SRC_BASEPATH "/2", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[2] = open(PRELOAD_SRC_BASEPATH "/3", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[3] = open(PRELOAD_SRC_BASEPATH "/4", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[4] = open(PRELOAD_SRC_BASEPATH "/5", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[5] = open(PRELOAD_SRC_BASEPATH "/6", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    uint32_t  ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t  moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t  mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t  ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t  moov_sz_be = htobe32(moov_sz);
    uint32_t  mdat_sz_be = htobe32(mdat_sz);
    write(fds[0], (char *)&ftyp_sz_be, sizeof(uint32_t));
    write(fds[0], MP4_ATOM_FTYP, ftyp_sz - sizeof(uint32_t));
    uint32_t mdat_body_rd_offset = 0;
    { // mdat
        write(fds[0], (char *)&mdat_sz_be, sizeof(uint32_t));
        write(fds[1], MP4__ATOM_TYPE_MDAT, sizeof(uint32_t));
        mdat_body_rd_offset = lseek(fds[1], 0, SEEK_CUR);
        uint32_t body_tot_sz = strlen(MP4__ATOM_BODY_MDAT);
        const char *body_ptr = MP4__ATOM_BODY_MDAT ;
        write(fds[1], body_ptr, 11);
        body_ptr += 11;
        body_tot_sz -= 11;
        write(fds[2], body_ptr, 13);
        body_ptr += 13;
        body_tot_sz -= 13;
        write(fds[3], body_ptr, body_tot_sz);
    } { // moov
        char *moov_sz_be_str = (char *)&moov_sz_be;
        write(fds[3], &moov_sz_be_str[0], 3);
        write(fds[4], &moov_sz_be_str[3], 1);
        write(fds[4],  MP4__ATOM_TYPE_MOOV, sizeof(uint32_t));
        uint32_t body_tot_sz = strlen(MP4__ATOM_BODY_MOOV);
        const char *body_ptr = MP4__ATOM_BODY_MOOV ;
        write(fds[4], body_ptr, 7);
        body_ptr += 7;
        body_tot_sz -= 7;
        write(fds[5], body_ptr, body_tot_sz);
    } {
        asa_op_base_cfg_t *asa_src_cfg = mp4proc->super.data.src.storage.handle;
        uint32_t *mdat_info = calloc(3, sizeof(uint32_t));
        mdat_info[0] = strlen(MP4__ATOM_BODY_MDAT);
        mdat_info[1] = 1;
        mdat_info[2] = mdat_body_rd_offset;
        asa_src_cfg->cb_args.entries[2] = &mdat_info[0]; // size of mdat body
        asa_src_cfg->cb_args.entries[3] = &mdat_info[1]; // sequemce number of file chunk where mdat body starts
        asa_src_cfg->cb_args.entries[4] = &mdat_info[2]; // offset of the file chunk
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for(int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer( lseek(fds[idx], 0, SEEK_CUR) ));
        close(fds[idx]);
    }
} // end of preload_stream_info_ok_2__prepare_fchunks


Ensure(atfp_mp4_test__preload_stream_info_ok_2)
{ // `mdat` comes before `moov`
    atfp_mp4_t  mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_ok_2__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.src.storage.handle;
    uv_loop_t  *loop = mp4proc.super.data.loop ;
    ASA_RES_CODE  result = atfp_mp4__preload_stream_info( &mp4proc,
            utest_atfp_mp4__preload_stream_info__done_ok );
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t done_flag = 0;
    asa_src_cfg->cb_args.entries[1] = &done_flag;
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    free(asa_src_cfg->cb_args.entries[2]);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info_ok_2
#undef  NUM_FILE_CHUNKS


static void utest_atfp_mp4__preload_stream_info__done_error(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info  = processor->data.error;
    assert_that(json_object_size(err_info), is_greater_than(0));
    asa_op_base_cfg_t  *asa_src_cfg = processor->data.src.storage.handle;
    uint8_t *done_flag = asa_src_cfg->cb_args.entries[1];
    *done_flag = 1;
}

#define  NUM_FILE_CHUNKS  3
static void preload_stream_info_corrupt_moov__prepare_fchunks (atfp_mp4_t *mp4proc)
{
    int fds[NUM_FILE_CHUNKS] = {0};
    fds[0] = open(PRELOAD_SRC_BASEPATH "/1", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[1] = open(PRELOAD_SRC_BASEPATH "/2", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    fds[2] = open(PRELOAD_SRC_BASEPATH "/3", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    uint32_t  ftyp_sz = strlen(MP4_ATOM_FTYP) + sizeof(uint32_t);
    uint32_t  moov_sz = strlen(MP4_ATOM_MOOV) + sizeof(uint32_t);
    uint32_t  mdat_sz = strlen(MP4_ATOM_MDAT) + sizeof(uint32_t);
    uint32_t  ftyp_sz_be = htobe32(ftyp_sz);
    uint32_t  moov_sz_be = htobe32(moov_sz);
    uint32_t  mdat_sz_be = htobe32(mdat_sz);
    {
        write(fds[0], (char *)&ftyp_sz_be, sizeof(uint32_t));
        write(fds[0], MP4_ATOM_FTYP, ftyp_sz - sizeof(uint32_t));
        write(fds[1], (char *)&moov_sz_be, sizeof(uint32_t));
        write(fds[1], MP4_ATOM_MOOV, moov_sz - sizeof(uint32_t) - 1); // corruption starts at here
        write(fds[2], (char *)&mdat_sz_be, sizeof(uint32_t));
        write(fds[2], MP4_ATOM_MDAT, mdat_sz - sizeof(uint32_t));
    }
    json_t *parts_size = json_object_get(mp4proc->super.data.spec, "parts_size");
    for(int idx = 0; idx < NUM_FILE_CHUNKS; idx++) {
        json_array_append_new(parts_size, json_integer( lseek(fds[idx], 0, SEEK_CUR) ));
        close(fds[idx]);
    }
} // end of preload_stream_info_corrupt_moov__prepare_fchunks

Ensure(atfp_mp4_test__preload_stream_info__corrupted_moov) {
    atfp_mp4_t  mp4proc = {0};
    utest_init_mp4_preload(&mp4proc, preload_stream_info_corrupt_moov__prepare_fchunks);
    asa_op_base_cfg_t *asa_src_cfg = mp4proc.super.data.src.storage.handle;
    uv_loop_t  *loop = mp4proc.super.data.loop ;
    ASA_RES_CODE  result = atfp_mp4__preload_stream_info( &mp4proc,
            utest_atfp_mp4__preload_stream_info__done_error );
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uint8_t done_flag = 0;
    asa_src_cfg->cb_args.entries[1] = &done_flag;
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    utest_deinit_mp4_preload(&mp4proc);
} // end of atfp_mp4_test__preload_stream_info__corrupted_moov
#undef  NUM_FILE_CHUNKS


TestSuite *app_transcoder_mp4_preload_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_mp4_test__preload_stream_info_ok_1);
    add_test(suite, atfp_mp4_test__preload_stream_info_ok_2);
    add_test(suite, atfp_mp4_test__preload_stream_info__corrupted_moov);
    return suite;
}
