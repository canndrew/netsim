// needed for SIOCADDRT
#include <sys/ioctl.h>

// needed for ifreq
#include <net/if.h>

// needed for lots of symbols
#include <net/if_arp.h>

// needed for rtentry RTF_UP RTF_GATEWAY
#include <net/route.h>


#include <linux/if_tun.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <pwd.h>
#include <grp.h>
#include <sys/prctl.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>

