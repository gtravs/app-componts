
use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};

use super::copy::{CopyBuffer, Direction};
use super::prelude::{Request, Response};

enum TransferState {
    Running(CopyBuffer),
    ShuttingDown(u64,String),
    Done(u64,String),
}

fn transfer_one_direction<A, B>(
    cx: &mut Context<'_>,
    state: &mut TransferState,
    r: &mut A,
    w: &mut B,
) -> Poll<io::Result<(u64,String)>>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut r = Pin::new(r);
    let mut w = Pin::new(w);

    loop {
        match state {
            TransferState::Running(buf) => {
                //let count = ready!(buf.poll_copy(cx, r.as_mut(), w.as_mut()))?;
                let result = ready!(buf.poll_copy(cx, r.as_mut(), w.as_mut()));
                let count = match result {
                    Ok(count) => count,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        // 如果是 UnexpectedEof，使用已经读取的数据
                        if let Ok(raw_data) = String::from_utf8((&buf.buf[..]).to_vec()) {
                            let len = raw_data.len() as u64;
                            *state = TransferState::ShuttingDown(len, raw_data);
                            continue;
                        } else {
                            return Poll::Ready(Err(e));
                        }
                    }
                    Err(e) => return Poll::Ready(Err(e)),
                };
                // 打印每次拷贝的字节数
                //println!("[INFO] Copied {:?} bytes", count);
                //println!("[INFO] DATA:\n{:?}",String::from_utf8(((&buf.buf[..count as usize])).to_vec())git rm -r --cached target/);
                let raw_data = String::from_utf8(((&buf.buf[..count as usize])).to_vec()).unwrap();
                match buf.direction {
                    Direction::Request => {        
                        //let data  = Request::from_string(&raw_data).unwrap();
                        
                        let req = Request::from_string(&raw_data).unwrap();
                        //println!("[INFO 111] DATA:\n{:?}",req);
                        
                    }
                    Direction::Response => {
                        let raw_data = String::from_utf8(((&buf.buf[..count as usize])).to_vec()).unwrap();
                        //println!("[INFO 222] DATA:\n{:?}",String::from_utf8(((&buf.buf[..count as usize])).to_vec()));
                        //let resp  = Response::from_string(&raw_data).unwrap();
                        //println!("[INFO 222] DATA:\n{:?}",raw_data);

                        
                    }
                }
                *state = TransferState::ShuttingDown(count,raw_data);
            }

            // TransferState::ShuttingDown(count,raw_data) => {
            //     ready!(w.as_mut().poll_shutdown(cx))?;
            //     *state = TransferState::Done(*count,raw_data);
            // }
            TransferState::ShuttingDown(count, raw_data) => {
                // 处理关闭连接时的 UnexpectedEof
                match ready!(w.as_mut().poll_shutdown(cx)) {
                    Ok(_) => {},
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {},
                    Err(e) => return Poll::Ready(Err(e)),
                }
                *state = TransferState::Done(*count, raw_data.clone());
            },
            TransferState::Done(count,raw_data) => return Poll::Ready(Ok((*count, raw_data.clone())))
        }
    }
}

pub async fn copy_bidirectional<A, B>(a: &mut A, b: &mut B) -> io::Result<((u64,String), (u64,String))>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    copy_bidirectional_impl(
        a,
        b,
        CopyBuffer::new(super::http_proxy::DEFAULT_BUF_SIZE, Direction::Request),
        CopyBuffer::new(super::http_proxy::DEFAULT_BUF_SIZE, Direction::Response),
    )
    .await
}

/// Copies data in both directions between `a` and `b` using buffers of the specified size.
///
/// This method is the same as the [`copy_bidirectional()`], except that it allows you to set the
/// size of the internal buffers used when copying data.
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub async fn copy_bidirectional_with_sizes<A, B>(

    a: &mut A,
    b: &mut B,
    a_to_b_buf_size: usize,
    b_to_a_buf_size: usize,
) -> io::Result<((u64,String), (u64,String))>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    copy_bidirectional_impl(
        a,
        b,
        CopyBuffer::new(a_to_b_buf_size,Direction::Request),
        CopyBuffer::new(b_to_a_buf_size,Direction::Response),
    )
    .await
}

async fn copy_bidirectional_impl<A, B>(
    a: &mut A,
    b: &mut B,
    a_to_b_buffer: CopyBuffer,
    b_to_a_buffer: CopyBuffer,
) -> io::Result<((u64,String), (u64,String))>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut a_to_b = TransferState::Running(a_to_b_buffer);
    let mut b_to_a = TransferState::Running(b_to_a_buffer);
    poll_fn(|cx| {
        let (a_to_b) = transfer_one_direction(cx, &mut a_to_b, a, b)?;
        let b_to_a = transfer_one_direction(cx, &mut b_to_a, b, a)?;

        // It is not a problem if ready! returns early because transfer_one_direction for the
        // other direction will keep returning TransferState::Done(count) in future calls to poll
        let a_to_b = ready!(a_to_b);
        let b_to_a = ready!(b_to_a);

        Poll::Ready(Ok((a_to_b, b_to_a)))
    })
    .await
}


