#ifndef INTERCOM_CPP_POSIX_ICLASSFACTORY_H
#define INTERCOM_CPP_POSIX_ICLASSFACTORY_H

#include "iunknown.h"

// MIDL_INTERFACE("00000001-0000-0000-C000-000000000046")
struct IClassFactory : public IUnknown
{
public:

    virtual HRESULT CreateInstance(
        IUnknown *pUnkOuter,
        REFIID riid,
        void **ppvObject
    ) = 0;

    virtual HRESULT LockServer(
        BOOL fLock
    ) = 0;

};

static const GUID IID_IClassFactory = { 0x00000001, 0x0000, 0x0000, { 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  0x46 } };

#endif